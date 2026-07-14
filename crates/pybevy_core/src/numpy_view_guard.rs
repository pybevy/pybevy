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
    counter: Arc<AtomicUsize>,
    released: AtomicBool,
    _owner: Py<PyAny>,
}

impl PyNumpyViewGuard {
    /// Increment `counter` and return a guard that decrements it on drop.
    pub fn acquire(counter: Arc<AtomicUsize>, owner: Py<PyAny>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self {
            counter,
            released: AtomicBool::new(false),
            _owner: owner,
        }
    }

    /// Release the counted view early (idempotent).
    pub fn release(&self) {
        if !self.released.swap(true, Ordering::AcqRel) {
            self.counter.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl Drop for PyNumpyViewGuard {
    fn drop(&mut self) {
        self.release();
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
    use super::*;

    #[test]
    fn guard_decrements_on_drop() {
        pyo3::Python::initialize();
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
        pyo3::Python::initialize();
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
        pyo3::Python::initialize();
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
}
