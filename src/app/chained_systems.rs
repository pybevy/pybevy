use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyTuple,
};

use crate::ecs::system_config::{PySystemSetConfig, system_set_value};

#[pyclass(name = "ChainedSystems", from_py_object)]
pub struct PyChainedSystems {
    pub systems: Py<PyTuple>,
}

impl Clone for PyChainedSystems {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            systems: self.systems.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyChainedSystems {
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.systems)
    }

    #[new]
    #[pyo3(signature = (*systems))]
    pub fn new(systems: Bound<'_, PyTuple>) -> Self {
        Self {
            systems: systems.unbind(),
        }
    }
}

/// A sequence of system sets configured with Bevy's chained ordering semantics.
#[pyclass(name = "ChainedSystemSets", from_py_object)]
pub struct PyChainedSystemSets {
    pub sets: Py<PyTuple>,
}

impl Clone for PyChainedSystemSets {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            sets: self.sets.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyChainedSystemSets {
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.sets)
    }

    #[new]
    #[pyo3(signature = (*sets))]
    pub fn new(sets: Bound<'_, PyTuple>) -> Self {
        Self {
            sets: sets.unbind(),
        }
    }
}

#[pyfunction]
#[pyo3(signature = (*systems))]
pub fn chain(py: Python<'_>, systems: Bound<'_, PyTuple>) -> PyResult<Py<PyAny>> {
    if systems.is_empty() {
        return Err(PyValueError::new_err(
            "chain() requires at least one system",
        ));
    }

    let mut set_count = 0;
    for value in systems.iter() {
        if value.extract::<PySystemSetConfig>().is_ok() || system_set_value(&value)?.is_some() {
            set_count += 1;
        }
    }

    if set_count == systems.len() {
        return Ok(Py::new(py, PyChainedSystemSets::new(systems))?.into_any());
    }
    if set_count != 0 {
        return Err(PyTypeError::new_err(
            "chain() cannot mix system sets with systems",
        ));
    }

    Ok(Py::new(py, PyChainedSystems::new(systems))?.into_any())
}
