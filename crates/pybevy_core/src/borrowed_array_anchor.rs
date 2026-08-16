//! Liveness probe backing zero-copy bounded arrays over asset data.
//!
//! An [`AssetBorrowAnchor`] is the concrete [`BorrowProbe`] that
//! `pybevy_array`'s borrowed storage consults on every operation. It bundles:
//!
//! - the borrowed asset's `ValidityFlag` (or `None` for a Python-owned asset,
//!   which is always live), so an escaped array raises a clean error once the
//!   owning system finishes or if accessed cross-thread; and
//! - a [`PyNumpyViewGuard`], which holds a read-view count on the asset (blocking
//!   mutation while the array is alive) and a strong reference to the owning
//!   Python object (keeping owned asset data alive). Dropping the anchor (when
//!   the last Python array referencing it is deallocated) releases the count.
//!
//! **Thread affinity.** Every anchor is pinned to the thread that created it and
//! rejects access from any other thread. For asset borrows the `ValidityFlag` is
//! already thread-affine; the explicit pin also covers Python-*owned* assets
//! (`validity = None`), which otherwise have no affinity. Under free-threaded
//! Python this is what keeps a mutable array's per-operation writes from racing
//! a `close()`/reallocation on another thread: only the owning thread can
//! operate or close, so an in-flight write always holds the write count and
//! blocks reallocation.

use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    thread::ThreadId,
};

use pybevy_array::BorrowProbe;
use pybevy_storage::{PendingViewClaim, ValidityFlag};

use crate::numpy_view_guard::{PendingNumpyViewGuard, PyNumpyViewGuard};

fn on_owner_thread(owner: ThreadId) -> bool {
    std::thread::current().id() == owner
}

const CROSS_THREAD: &str = "borrowed array accessed from a different thread than it was created on";

pub struct AssetBorrowAnchor {
    validity: Option<ValidityFlag>,
    owner: ThreadId,
    guard: PyNumpyViewGuard,
    closed: AtomicBool,
}

impl AssetBorrowAnchor {
    pub fn new(validity: Option<ValidityFlag>, guard: PyNumpyViewGuard) -> Self {
        Self {
            validity,
            owner: std::thread::current().id(),
            guard,
            closed: AtomicBool::new(false),
        }
    }

    pub fn close(&self) {
        if !on_owner_thread(self.owner) {
            return;
        }
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.guard.release();
        }
    }
}

impl fmt::Debug for AssetBorrowAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetBorrowAnchor")
            .field("borrowed_asset", &self.validity.is_some())
            .field("closed", &self.closed.load(Ordering::Acquire))
            .finish()
    }
}

impl BorrowProbe for AssetBorrowAnchor {
    fn check_read(&self) -> Result<(), String> {
        if !on_owner_thread(self.owner) {
            return Err(CROSS_THREAD.to_string());
        }
        if self.closed.load(Ordering::Acquire) {
            return Err("array is closed after its context exited".to_string());
        }
        match &self.validity {
            None => Ok(()),
            Some(flag) => flag.check_read().map_err(|e| {
                format!(
                    "the owning system has finished or access crossed threads ({e}); \
                     call .copy() inside the system to keep an independent snapshot"
                )
            }),
        }
    }
}

/// Probe for an in-place *mutable* borrow. Adds a `closed` flag (set when the
/// mutable context exits) and permits writes while live via `check_write`.
pub struct AssetBorrowAnchorMut {
    validity: Option<ValidityFlag>,
    owner: ThreadId,
    guard: PendingNumpyViewGuard,
    closed: AtomicBool,
}

impl AssetBorrowAnchorMut {
    pub fn new(validity: Option<ValidityFlag>, guard: PendingNumpyViewGuard) -> Self {
        Self {
            validity,
            owner: std::thread::current().id(),
            guard,
            closed: AtomicBool::new(false),
        }
    }

    /// Publish a view whose Python owner and array storage are fully bound.
    pub fn commit(&self) {
        self.guard.commit();
    }

    pub fn pending_claim(&self) -> &PendingViewClaim {
        self.guard.claim()
    }

    /// Close the borrow (idempotent): after this, all reads and writes on the
    /// array raise, and the exclusive write count is released. Called from the
    /// mutable context's `__exit__`.
    ///
    /// A cross-thread call is a no-op: releasing the write count is deferred to
    /// `Drop` (which only runs at refcount zero, when no operation is in
    /// flight), so a stray `__exit__` on another thread can never release the
    /// lock while the owning thread is mid-write.
    pub fn close(&self) {
        if !on_owner_thread(self.owner) {
            return;
        }
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.guard.release();
        }
    }
}

impl fmt::Debug for AssetBorrowAnchorMut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetBorrowAnchorMut")
            .field("borrowed_asset", &self.validity.is_some())
            .field("closed", &self.closed.load(Ordering::Acquire))
            .finish()
    }
}

impl AssetBorrowAnchorMut {
    fn check_live(&self, write: bool) -> Result<(), String> {
        if !on_owner_thread(self.owner) {
            return Err(CROSS_THREAD.to_string());
        }
        if self.closed.load(Ordering::Acquire) {
            return Err("array is closed after its mutable context exited".to_string());
        }
        match &self.validity {
            None => Ok(()),
            Some(flag) => {
                let checked = if write {
                    flag.check_write()
                } else {
                    flag.check_read()
                };
                checked.map_err(|e| {
                    format!("the owning system has finished or access crossed threads ({e})")
                })
            }
        }
    }
}

impl BorrowProbe for AssetBorrowAnchorMut {
    fn check_read(&self) -> Result<(), String> {
        self.check_live(false)
    }

    fn check_write(&self) -> Result<(), String> {
        self.check_live(true)
    }
}
