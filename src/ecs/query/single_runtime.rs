use std::{collections::HashMap, sync::Arc};

use bevy::ecs::component::ComponentId;
use pyo3::{exceptions::PyRuntimeError, ffi::PyTypeObject, prelude::*};

use crate::ecs::{
    helpers::validity_guard::ValidityFlag,
    query::{query_param::PyQueryParam, query_runtime::PyQueryIter},
};

/// Runtime wrapper for Single<T> queries that enforces exactly one entity matches.
///
/// This wraps a PyQueryIter but validates that exactly one entity exists matching
/// the query filter before returning the result.
#[pyclass(name = "SingleQuery")]
pub struct PySingleQuery {
    /// The underlying query iterator
    query_iter: Py<PyQueryIter>,
    /// Whether we've already returned the single result
    returned: bool,
    /// Cached single item for __getattr__/__setattr__ delegation (Deref emulation)
    cached_item: Option<Py<PyAny>>,
}

impl PySingleQuery {
    /// Creates a new Single query wrapper
    ///
    /// # Safety
    /// The world pointer must remain valid for the lifetime of this object
    pub unsafe fn new(
        param: Arc<PyQueryParam>,
        world: &mut bevy::prelude::World,
        custom_component_ids: Arc<HashMap<*const PyTypeObject, ComponentId>>,
        validity: ValidityFlag,
    ) -> Self {
        // SAFETY: Caller guarantees world pointer is valid during system execution
        let query_iter = unsafe { PyQueryIter::new(param, world, custom_component_ids, validity) };

        Python::attach(|py| {
            let query_iter_py = Py::new(py, query_iter).expect("Failed to create PyQueryIter");

            PySingleQuery {
                query_iter: query_iter_py,
                returned: false,
                cached_item: None,
            }
        })
    }
}

#[pymethods]
impl PySingleQuery {
    /// Makes this object iterable (but will only yield one item)
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the single query result, panicking if zero or multiple entities match
    fn __next__(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        if self.returned {
            return Err(pyo3::exceptions::PyStopIteration::new_err(""));
        }

        // Get the first result
        let first = self.query_iter.call_method0(py, "__next__")?;

        // Check if there's a second result (which would be an error)
        match self.query_iter.call_method0(py, "__next__") {
            Ok(_) => {
                // There's a second result - this is an error
                Err(PyRuntimeError::new_err(
                    "Single<T> query matched multiple entities. Expected exactly one entity.",
                ))
            }
            Err(e) if e.is_instance_of::<pyo3::exceptions::PyStopIteration>(py) => {
                // No second result - this is correct, return the first
                self.returned = true;
                Ok(first)
            }
            Err(e) => {
                // Some other error
                Err(e)
            }
        }
    }

    /// Delegate attribute access to the single result, matching Bevy's Deref behavior.
    /// In Bevy Rust, Single<T> implements Deref<Target=T>, so `single.field` works directly.
    fn __getattr__(&mut self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        let item = self.get_or_fetch_item(py)?;
        item.getattr(py, name)
    }

    /// Delegate attribute setting to the single result (Bevy's DerefMut).
    fn __setattr__(&mut self, py: Python, name: &str, value: Py<PyAny>) -> PyResult<()> {
        let item = self.get_or_fetch_item(py)?;
        item.setattr(py, name, value)
    }
}

impl PySingleQuery {
    /// Get the single item, fetching it on first access and caching it.
    fn get_or_fetch_item(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        if let Some(ref item) = self.cached_item {
            return Ok(item.clone_ref(py));
        }

        // Get the first result
        let first = self.query_iter.call_method0(py, "__next__")?;

        // Verify exactly one entity matches
        match self.query_iter.call_method0(py, "__next__") {
            Ok(_) => {
                return Err(PyRuntimeError::new_err(
                    "Single<T> query matched multiple entities. Expected exactly one entity.",
                ));
            }
            Err(e) if e.is_instance_of::<pyo3::exceptions::PyStopIteration>(py) => {
                // Correct - exactly one entity
            }
            Err(e) => return Err(e),
        }

        self.returned = true;
        self.cached_item = Some(first.clone_ref(py));
        Ok(first)
    }
}
