use pyo3::{exceptions::PyNotImplementedError, prelude::*};

/// Base class for types that can be converted to materials.
///
/// This is a marker class for the Python type hierarchy.
/// Subclasses (like Color) can be used in material contexts.
#[pyclass(name = "Materializable", subclass, frozen)]
#[derive(Debug, Clone)]
pub struct PyMaterializable;

#[pymethods]
impl PyMaterializable {
    pub fn materialize(&self, _py: Python<'_>) -> PyResult<Py<PyAny>> {
        Err(PyNotImplementedError::new_err(
            "Subclasses of Materializable must implement materialize()",
        ))
    }
}
