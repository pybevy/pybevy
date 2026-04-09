use pyo3::{exceptions::PyValueError, prelude::*, types::PyTuple};

#[pyclass(name = "ChainedSystems")]
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
    pub fn new(systems: Bound<'_, PyTuple>) -> Self {
        Self {
            systems: systems.unbind(),
        }
    }
}

#[pyfunction]
#[pyo3(signature = (*systems))]
pub fn chain(systems: Bound<'_, PyTuple>) -> PyResult<PyChainedSystems> {
    if systems.is_empty() {
        return Err(PyValueError::new_err(
            "chain() requires at least one system",
        ));
    }
    Ok(PyChainedSystems::new(systems))
}
