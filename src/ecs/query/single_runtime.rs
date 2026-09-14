use bevy::ecs::{change_detection::Tick, world::unsafe_world_cell::UnsafeWorldCell};
use pybevy_core::public_error::{SINGLE_NOT_ITERABLE, SINGLE_NOT_SUBSCRIPTABLE};
use pyo3::{PyTraverseError, PyVisit, exceptions::PyTypeError, prelude::*};

use crate::ecs::{
    helpers::validity_guard::ValidityFlag,
    query::query_runtime::{CachedQuery, PyQueryIter, query_execution_error_to_py},
};

/// Runtime wrapper for Single<T> queries that enforces exactly one entity matches.
///
/// This wraps a PyQueryIter but validates that exactly one entity exists matching
/// the query filter before returning the result.
#[pyclass(name = "SingleQuery", module = "pybevy.ecs")]
pub struct PySingleQuery {
    /// The underlying query iterator
    query_iter: Py<PyQueryIter>,
    /// Cached single item for repeatable extraction
    cached_item: Option<Py<PyAny>>,
}

impl PySingleQuery {
    pub(crate) fn matching_count(&self, py: Python<'_>) -> PyResult<usize> {
        self.query_iter.borrow(py).matching_count()
    }

    /// Creates a new Single query wrapper
    ///
    /// # Safety
    /// `cached` must remain valid and `world_cell` must reference the World it was
    /// built from, for the lifetime of this object (fenced by `validity`).
    pub unsafe fn new(
        cached: &CachedQuery,
        world_cell: UnsafeWorldCell,
        validity: ValidityFlag,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        // SAFETY: Caller guarantees the cached state and world cell are valid during
        // system execution (see PyQueryIter::new safety contract).
        let query_iter =
            unsafe { PyQueryIter::new(cached, world_cell, validity, last_run, this_run) };

        Python::attach(|py| {
            let query_iter_py = Py::new(py, query_iter).expect("Failed to create PyQueryIter");

            PySingleQuery {
                query_iter: query_iter_py,
                cached_item: None,
            }
        })
    }
}

#[pymethods]
impl PySingleQuery {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.query_iter)?;
        visit.call(&self.cached_item)
    }

    /// Return the underlying row with its existing borrowed access.
    #[allow(
        clippy::wrong_self_convention,
        reason = "Python extraction keeps the holder reusable"
    )]
    fn into_inner(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.get_or_fetch_item(py)
    }

    fn __iter__(&self) -> PyResult<()> {
        Err(PyTypeError::new_err(SINGLE_NOT_ITERABLE))
    }

    fn __getitem__(&self, _index: Py<PyAny>) -> PyResult<()> {
        Err(PyTypeError::new_err(SINGLE_NOT_SUBSCRIPTABLE))
    }
}

impl PySingleQuery {
    /// Get the single item, fetching it on first access and caching it.
    fn get_or_fetch_item(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        self.query_iter.borrow(py).check_valid()?;
        if let Some(ref item) = self.cached_item {
            return Ok(item.clone_ref(py));
        }

        let first = self.fetch_single(py)?;
        self.cached_item = Some(first.clone_ref(py));
        Ok(first)
    }

    fn fetch_single(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        self.query_iter
            .borrow(py)
            .materialize_single(py)
            .map_err(query_execution_error_to_py)
    }
}
