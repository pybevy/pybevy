//! Deferred destruction of retired Python component and resource values.
//!
//! Bevy calls the custom Python object drop function while its storage is in
//! the middle of a mutation: `BlobArray::replace_unchecked` resolves the
//! destination pointer, drops the old value, and only then copies the
//! replacement into that destination. A decref inside that window can run a
//! Python `__del__` whose callback reenters the World and reallocates the same
//! table, so the subsequent copy writes into freed storage. Dropping Python
//! values must therefore never run Python code inline during a native
//! mutation.
//!
//! The fix defers the decref into a thread-local queue and drains it at the
//! boundary of the enclosing native mutation window, where no storage pointer
//! is in flight. Drops on an unattached thread keep pyo3's reference-pool
//! path, which already runs finalizers only at the next attachment.

use std::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
};

use pyo3::{Py, types::PyAny};

use super::world_gc::WorldGcGuard;

thread_local! {
    static DEFERRED_PY_DROPS: RefCell<Vec<Py<PyAny>>> = const { RefCell::new(Vec::new()) };
}

/// Process-wide count of queued values. A hint only: it lets `flush` skip the
/// attach entirely in the common empty case. A thread may observe a nonzero
/// count that belongs to another thread's queue; draining its own empty queue
/// is harmless.
static DEFERRED_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Take ownership of a Python value removed from Bevy storage and delay its
/// decref until the next flush boundary.
pub(crate) fn defer_py_drop(value: Py<PyAny>) {
    // SAFETY: `PyGILState_Check` reports whether this thread is attached and
    // has no preconditions. Unattached drops keep pyo3's reference-pool
    // behavior: the decref runs at the next attachment, never inline.
    if unsafe { pyo3::ffi::PyGILState_Check() } == 0 {
        drop(value);
        return;
    }
    DEFERRED_PY_DROPS.with(|drops| drops.borrow_mut().push(value));
    DEFERRED_DROP_COUNT.fetch_add(1, Ordering::AcqRel);
}

/// Run every deferred decref collected on this thread.
///
/// Safe to call at any boundary where no Bevy storage pointer is in flight,
/// including from inside a flush (a finalizer's own World calls drain nothing
/// new). Finalizer exceptions surface through Python's unraisable hook.
pub(crate) fn flush_deferred_py_drops() {
    if DEFERRED_DROP_COUNT.load(Ordering::Acquire) == 0 || std::thread::panicking() {
        return;
    }
    pyo3::Python::attach(|_| {
        loop {
            let batch = DEFERRED_PY_DROPS.with(|drops| std::mem::take(&mut *drops.borrow_mut()));
            if batch.is_empty() {
                break;
            }
            DEFERRED_DROP_COUNT.fetch_sub(batch.len(), Ordering::AcqRel);
            for value in batch {
                drop(value);
            }
        }
    });
}

/// RAII flush at the end of a native mutation window: its drop runs every
/// queued finalizer once the guarded borrow is gone and no storage pointer is
/// in flight. Constructing the guard has no side effects; the enclosing
/// boundary performs the pre-mutation drain explicitly.
pub(crate) struct MutationFlushGuard;

impl Drop for MutationFlushGuard {
    fn drop(&mut self) {
        flush_deferred_py_drops();
    }
}

/// Mutable World access whose scope is a native mutation window.
///
/// The caller performs the validity check before calling `new`; the guard's
/// scope is what flushes retired Python values at the window boundaries.
pub(crate) struct WorldMutGuard<'a> {
    world: &'a mut bevy::ecs::world::World,
    _flush: MutationFlushGuard,
    _gc: Option<WorldGcGuard>,
}

impl<'a> WorldMutGuard<'a> {
    pub(crate) fn new(world: &'a mut bevy::ecs::world::World) -> Self {
        // Contract: drain residue from any earlier window before this
        // mutation starts, so its finalizers never run inside this window.
        flush_deferred_py_drops();
        Self {
            world,
            _flush: MutationFlushGuard,
            _gc: None,
        }
    }

    pub(crate) fn with_gc(
        world: &'a mut bevy::ecs::world::World,
        gc: Option<WorldGcGuard>,
    ) -> Self {
        let mut guard = Self::new(world);
        guard._gc = gc;
        guard
    }
}

impl<'a> std::ops::Deref for WorldMutGuard<'a> {
    type Target = bevy::ecs::world::World;

    fn deref(&self) -> &Self::Target {
        self.world
    }
}

impl<'a> std::ops::DerefMut for WorldMutGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}
