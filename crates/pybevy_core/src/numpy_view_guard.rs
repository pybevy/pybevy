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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use pybevy_storage::ViewCounters;
    use pyo3::ffi::Py_REFCNT;

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

    #[test]
    fn release_array_guard_releases_only_a_guard_base() {
        Python::initialize();
        let counter = Arc::new(AtomicUsize::new(0));
        Python::attach(|py| {
            let guard = Py::new(
                py,
                PyNumpyViewGuard::acquire(counter.clone(), py.None().into_any()),
            )
            .unwrap();
            let namespace = py
                .import("types")
                .unwrap()
                .getattr("SimpleNamespace")
                .unwrap();

            // A guard base releases the counted view, exactly once.
            let with_guard: Bound<PyAny> = namespace.call0().unwrap();
            with_guard.setattr("base", guard.clone_ref(py)).unwrap();
            assert_eq!(counter.load(Ordering::Acquire), 1);
            release_array_guard(&with_guard);
            assert_eq!(counter.load(Ordering::Acquire), 0);
            release_array_guard(&with_guard);
            assert_eq!(counter.load(Ordering::Acquire), 0);

            // A missing base and a foreign base are no-ops.
            let plain: Bound<PyAny> = namespace.call0().unwrap();
            release_array_guard(&plain);
            assert_eq!(counter.load(Ordering::Acquire), 0);
            let foreign: Bound<PyAny> = namespace.call0().unwrap();
            foreign.setattr("base", 42i32).unwrap();
            release_array_guard(&foreign);
            assert_eq!(counter.load(Ordering::Acquire), 0);
        });
    }

    #[test]
    fn guard_retains_exactly_one_owner_reference_until_drop() {
        Python::initialize();
        Python::attach(|py| {
            let owner: Bound<PyAny> = py.eval(c"object()", None, None).unwrap();
            // Local reference + the guard's _owner: exactly two.
            let guard = Py::new(
                py,
                PyNumpyViewGuard::acquire(Arc::new(AtomicUsize::new(0)), owner.clone().unbind()),
            )
            .unwrap();
            // SAFETY: owner is alive for the call; Py_REFCNT only reads its header.
            assert_eq!(unsafe { Py_REFCNT(owner.as_ptr()) }, 2);
            drop(guard);
            // Nothing is retained beyond the local reference: no leak.
            // SAFETY: same as above; the object is still alive.
            assert_eq!(unsafe { Py_REFCNT(owner.as_ptr()) }, 1);
        });
    }

    // Restores the caller's gc state (isenabled/threshold) even on assert panic.
    struct GcStateRestore {
        enabled: bool,
        threshold: (i32, i32, i32),
    }

    impl GcStateRestore {
        fn disable(py: Python<'_>) -> Self {
            let gc = py.import("gc").unwrap();
            let enabled: bool = gc.call_method0("isenabled").unwrap().extract().unwrap();
            let threshold: (i32, i32, i32) =
                gc.call_method0("get_threshold").unwrap().extract().unwrap();
            gc.call_method0("disable").unwrap();
            Self { enabled, threshold }
        }
    }

    impl Drop for GcStateRestore {
        fn drop(&mut self) {
            Python::attach(|py| {
                let gc = py.import("gc").unwrap();
                let (gen0, gen1, gen2) = self.threshold;
                gc.call_method1("set_threshold", (gen0, gen1, gen2))
                    .unwrap();
                if self.enabled {
                    gc.call_method0("enable").unwrap();
                } else {
                    gc.call_method0("disable").unwrap();
                }
            });
        }
    }

    #[test]
    fn cycled_view_is_released_when_the_cycle_collector_clears_the_cycle() {
        Python::initialize();
        let counter = Arc::new(AtomicUsize::new(0));
        Python::attach(|py| {
            // RAII restores the caller's collector state even on assert panic.
            let _gc = GcStateRestore::disable(py);
            let guard = Py::new(
                py,
                PyNumpyViewGuard::acquire(counter.clone(), py.None().into_any()),
            )
            .unwrap();
            // A tracked refcount cycle that is the last reference to the guard.
            let inner: Bound<PyAny> = py.eval(c"[]", None, None).unwrap();
            let holder: Bound<PyAny> = py.eval(c"[]", None, None).unwrap();
            inner.call_method1("append", (holder.clone(),)).unwrap();
            holder.call_method1("append", (inner.clone(),)).unwrap();
            holder
                .call_method1("append", (guard.clone_ref(py),))
                .unwrap();
            assert_eq!(counter.load(Ordering::Acquire), 1);
            drop(inner);
            drop(holder);
            drop(guard);
            // Refcounting alone cannot clear the cycle: the count stays held.
            assert_eq!(counter.load(Ordering::Acquire), 1);
            py.import("gc").unwrap().call_method0("collect").unwrap();
            // The collector clears the cycle, freeing the guard and releasing the count.
            assert_eq!(counter.load(Ordering::Acquire), 0);
        });
    }
}
