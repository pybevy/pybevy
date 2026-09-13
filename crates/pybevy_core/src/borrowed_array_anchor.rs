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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use pybevy_array::BorrowProbe;
    use pybevy_storage::{ValidityFlag, ValidityGuard, ViewCounters};
    use pyo3::prelude::*;

    use super::*;
    use crate::StorageError;

    #[test]
    fn read_anchor_gates_on_validity_and_close_state() {
        Python::initialize();
        let counter = Arc::new(AtomicUsize::new(0));
        Python::attach(|py| {
            let guard = PyNumpyViewGuard::acquire(counter.clone(), py.None().into_any());
            let anchor = AssetBorrowAnchor::new(None, guard);
            assert_eq!(counter.load(Ordering::Acquire), 1);
            assert!(anchor.check_read().is_ok());
            anchor.close();
            assert_eq!(
                anchor.check_read().unwrap_err(),
                "array is closed after its context exited"
            );
            anchor.close();
            assert_eq!(counter.load(Ordering::Acquire), 0);
        });

        Python::attach(|py| {
            let guard =
                PyNumpyViewGuard::acquire(Arc::new(AtomicUsize::new(0)), py.None().into_any());
            let flag = ValidityFlag::new_write();
            let system = ValidityGuard::new(flag.clone());
            let anchor = AssetBorrowAnchor::new(Some(flag), guard);
            assert!(anchor.check_read().is_ok());
            drop(system);
            let expected = format!(
                "the owning system has finished or access crossed threads ({}); \
                 call .copy() inside the system to keep an independent snapshot",
                StorageError::InvalidAccess
            );
            assert_eq!(anchor.check_read().unwrap_err(), expected);

            // A closed anchor reports the closed error even while the flag is live.
            let guard2 =
                PyNumpyViewGuard::acquire(Arc::new(AtomicUsize::new(0)), py.None().into_any());
            let flag2 = ValidityFlag::new_write();
            let _system2 = ValidityGuard::new(flag2.clone());
            let anchor2 = AssetBorrowAnchor::new(Some(flag2), guard2);
            anchor2.close();
            assert_eq!(
                anchor2.check_read().unwrap_err(),
                "array is closed after its context exited"
            );
        });

        // RAII: dropping the anchor without close() releases the view count.
        Python::attach(|py| {
            let counter = Arc::new(AtomicUsize::new(0));
            let guard = PyNumpyViewGuard::acquire(counter.clone(), py.None().into_any());
            let anchor = AssetBorrowAnchor::new(None, guard);
            drop(anchor);
            assert_eq!(counter.load(Ordering::Acquire), 0);
        });
    }

    #[test]
    fn mut_anchor_gates_writes_and_releases_the_write_count_exactly_once() {
        Python::initialize();
        Python::attach(|py| {
            let counters = ViewCounters::default();
            let pending = PendingNumpyViewGuard::from_acquired(
                counters.try_prepare_write().expect("write gate free"),
                py.None().into_any(),
            );
            let anchor = AssetBorrowAnchorMut::new(None, pending);
            assert_eq!(counters.write_count(), 1);
            assert!(anchor.check_read().is_ok());
            assert!(anchor.check_write().is_ok());
            // Commit flips the claim out of the pending (authorizing) state.
            let claim = anchor.pending_claim();
            assert!(claim.authorizes(&counters));
            anchor.commit();
            assert!(!claim.authorizes(&counters));
            anchor.close();
            assert_eq!(
                anchor.check_write().unwrap_err(),
                "array is closed after its mutable context exited"
            );
            anchor.close();
            assert_eq!(counters.write_count(), 0);
        });

        Python::attach(|py| {
            let counters = ViewCounters::default();
            let pending = PendingNumpyViewGuard::from_acquired(
                counters.try_prepare_write().expect("write gate free"),
                py.None().into_any(),
            );
            let flag = ValidityFlag::new_write();
            let system = ValidityGuard::new(flag.clone());
            let anchor = AssetBorrowAnchorMut::new(Some(flag), pending);
            assert!(anchor.check_write().is_ok());
            drop(system);
            let expected = format!(
                "the owning system has finished or access crossed threads ({})",
                StorageError::InvalidAccess
            );
            assert_eq!(anchor.check_write().unwrap_err(), expected);
            assert_eq!(anchor.check_read().unwrap_err(), expected);
        });

        // Rollback: dropping the anchor before commit releases the write gate.
        Python::attach(|py| {
            let counters = ViewCounters::default();
            let pending = PendingNumpyViewGuard::from_acquired(
                counters.try_prepare_write().expect("write gate free"),
                py.None().into_any(),
            );
            let anchor = AssetBorrowAnchorMut::new(None, pending);
            assert!(anchor.pending_claim().authorizes(&counters));
            drop(anchor);
            assert_eq!(counters.write_count(), 0);
            assert!(counters.try_prepare_write().is_some());
        });
    }

    #[test]
    fn anchor_rejects_a_foreign_thread_and_defers_release_to_drop() {
        Python::initialize();
        let counter = Arc::new(AtomicUsize::new(0));
        Python::attach(|py| {
            let guard = PyNumpyViewGuard::acquire(counter.clone(), py.None().into_any());
            let anchor = AssetBorrowAnchor::new(None, guard);
            let inner = counter.clone();
            std::thread::spawn(move || {
                assert_eq!(
                    anchor.check_read().unwrap_err(),
                    "borrowed array accessed from a different thread than it was created on"
                );
                // A stray cross-thread close() must not release the count early.
                anchor.close();
                assert_eq!(inner.load(Ordering::Acquire), 1);
            })
            .join()
            .unwrap();
            // The guard's Drop is what releases the count.
            assert_eq!(counter.load(Ordering::Acquire), 0);
        });

        Python::attach(|py| {
            let counters = ViewCounters::default();
            let pending = PendingNumpyViewGuard::from_acquired(
                counters.try_prepare_write().expect("write gate free"),
                py.None().into_any(),
            );
            let anchor = AssetBorrowAnchorMut::new(None, pending);
            let inner = counters.clone();
            std::thread::spawn(move || {
                assert_eq!(
                    anchor.check_write().unwrap_err(),
                    "borrowed array accessed from a different thread than it was created on"
                );
                anchor.close();
                assert_eq!(inner.write_count(), 1);
            })
            .join()
            .unwrap();
            assert_eq!(counters.write_count(), 0);
        });
    }

    #[test]
    fn live_anchors_exclude_the_opposite_view_kind_until_released() {
        Python::initialize();
        Python::attach(|py| {
            // A live read anchor keeps the write gate unacquirable.
            let counters = ViewCounters::default();
            let claim = counters.try_prepare_read().expect("readers free");
            let anchor = AssetBorrowAnchor::new(
                None,
                PyNumpyViewGuard::from_acquired(claim, py.None().into_any()),
            );
            assert_eq!(counters.read_count(), 1);
            assert!(counters.try_prepare_write().is_none());
            anchor.close();
            assert_eq!(counters.read_count(), 0);
            let write_claim = counters.try_prepare_write().expect("released readers");
            PendingNumpyViewGuard::from_acquired(write_claim, py.None().into_any()).release();

            // A live write anchor (pending and committed) keeps the read gate closed.
            let counters2 = ViewCounters::default();
            let pending = PendingNumpyViewGuard::from_acquired(
                counters2.try_prepare_write().expect("write gate free"),
                py.None().into_any(),
            );
            let anchor2 = AssetBorrowAnchorMut::new(None, pending);
            assert!(counters2.try_prepare_read().is_none());
            anchor2.commit();
            assert!(counters2.try_prepare_read().is_none());
            anchor2.close();
            assert!(counters2.try_prepare_read().is_some());
        });
    }
}
