//! Lifetime guard for zero-copy NumPy views over asset data
//!
//! A `PyNumpyViewGuard` is passed as the base object of every NumPy array that
//! aliases asset memory (mesh attributes, image data). NumPy holds a strong
//! reference to its base for the array's whole lifetime, so the guard's `Drop`
//! runs exactly when the array is deallocated. On CPython that is deterministic
//! refcount-zero deallocation, not scheduled garbage collection; a view kept in
//! a reference cycle keeps its counter held (and mutation blocked) until the
//! cycle collector frees it, which fails safe.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use pybevy_storage::{PendingViewClaim, ReadViewClaim};
use pyo3::prelude::*;

/// Base object for NumPy views over asset data. Holds the view counter it
/// incremented at creation and a strong reference to the owning Python object
/// (so owned asset data cannot be freed while the view is alive).
///
/// The counter is released either by [`release`](Self::release) (called from a
/// view context's `__exit__`) or by `Drop` when NumPy deallocates the array,
/// whichever comes first.
#[pyclass(name = "_NumpyViewGuard", frozen)]
pub struct PyNumpyViewGuard {
    claim: PyViewClaim,
    _owner: Py<PyAny>,
}

enum PyViewClaim {
    Counter {
        counter: Arc<AtomicUsize>,
        released: AtomicBool,
    },
    Read(ReadViewClaim),
}

impl PyNumpyViewGuard {
    /// Increment `counter` and return a guard that decrements it on drop.
    pub fn acquire(counter: Arc<AtomicUsize>, owner: Py<PyAny>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self {
            claim: PyViewClaim::Counter {
                counter,
                released: AtomicBool::new(false),
            },
            _owner: owner,
        }
    }

    /// Wrap a `counter` the caller has already incremented through the owning
    /// asset storage's view-acquisition API. Decrements on release/drop; does
    /// not increment again.
    pub fn from_acquired(claim: ReadViewClaim, owner: Py<PyAny>) -> Self {
        Self {
            claim: PyViewClaim::Read(claim),
            _owner: owner,
        }
    }

    /// Release the counted view early (idempotent).
    pub fn release(&self) {
        match &self.claim {
            PyViewClaim::Counter { counter, released } => {
                if !released.swap(true, Ordering::AcqRel) {
                    counter.fetch_sub(1, Ordering::AcqRel);
                }
            }
            PyViewClaim::Read(claim) => claim.release(),
        }
    }
}

impl Drop for PyNumpyViewGuard {
    fn drop(&mut self) {
        self.release();
    }
}

/// Construction-time guard for a mutable view that is not published yet.
pub struct PendingNumpyViewGuard {
    claim: PendingViewClaim,
    _owner: Py<PyAny>,
}

impl PendingNumpyViewGuard {
    pub fn from_acquired(claim: PendingViewClaim, owner: Py<PyAny>) -> Self {
        Self {
            claim,
            _owner: owner,
        }
    }

    pub fn claim(&self) -> &PendingViewClaim {
        &self.claim
    }

    pub fn commit(&self) {
        self.claim.commit();
    }

    pub fn release(&self) {
        self.claim.release();
    }
}

/// Release the view guard backing a NumPy array, if it has one.
///
/// Looks up the array's `base` object; a missing or foreign base is ignored so
/// this is safe to call on any object.
pub fn release_array_guard(array: &Bound<'_, PyAny>) {
    if let Ok(base) = array.getattr("base")
        && let Ok(guard) = base.cast::<PyNumpyViewGuard>()
    {
        guard.get().release();
    }
}

#[cfg(test)]
mod tests {
    use pybevy_storage::ViewCounters;

    use super::*;

    #[test]
    fn guard_decrements_on_drop() {
        Python::initialize();
        let counter = Arc::new(AtomicUsize::new(0));
        Python::attach(|py| {
            let owner = py.None();
            let guard = PyNumpyViewGuard::acquire(counter.clone(), owner.into_any());
            assert_eq!(counter.load(Ordering::Acquire), 1);
            drop(guard);
            assert_eq!(counter.load(Ordering::Acquire), 0);
        });
    }

    #[test]
    fn release_is_idempotent_and_prevents_double_decrement() {
        Python::initialize();
        let counter = Arc::new(AtomicUsize::new(0));
        Python::attach(|py| {
            let owner = py.None();
            let guard = PyNumpyViewGuard::acquire(counter.clone(), owner.into_any());
            assert_eq!(counter.load(Ordering::Acquire), 1);
            guard.release();
            assert_eq!(counter.load(Ordering::Acquire), 0);
            guard.release();
            assert_eq!(counter.load(Ordering::Acquire), 0);
            drop(guard);
        });
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }

    #[test]
    fn guard_drop_runs_when_py_object_freed() {
        Python::initialize();
        let counter = Arc::new(AtomicUsize::new(0));
        Python::attach(|py| {
            let owner = py.None();
            let guard = PyNumpyViewGuard::acquire(counter.clone(), owner.into_any());
            let obj = Py::new(py, guard).unwrap();
            assert_eq!(counter.load(Ordering::Acquire), 1);
            drop(obj);
        });
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }

    #[test]
    fn pending_guard_rolls_back_or_commits_exactly_once() {
        Python::initialize();
        Python::attach(|py| {
            let counters = ViewCounters::default();
            let pending = PendingNumpyViewGuard::from_acquired(
                counters.try_prepare_write().expect("pending claim"),
                py.None().into_any(),
            );
            drop(pending);
            assert_eq!(counters.write_count(), 0);

            let ready = PendingNumpyViewGuard::from_acquired(
                counters.try_prepare_write().expect("pending claim"),
                py.None().into_any(),
            );
            ready.commit();
            ready.release();
            ready.release();
            assert_eq!(counters.write_count(), 0);
        });
    }

    #[test]
    fn acquired_read_claim_releases_exactly_once() {
        Python::initialize();
        Python::attach(|py| {
            let counters = ViewCounters::default();
            let claim = counters.try_prepare_read().expect("read claim");
            let guard = PyNumpyViewGuard::from_acquired(claim, py.None().into_any());
            assert_eq!(counters.read_count(), 1);
            guard.release();
            guard.release();
            assert_eq!(counters.read_count(), 0);
        });
    }
}
