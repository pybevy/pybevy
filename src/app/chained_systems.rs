use pyo3::{
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
