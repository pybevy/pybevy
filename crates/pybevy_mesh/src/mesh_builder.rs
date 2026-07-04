use pyo3::{exceptions::PyNotImplementedError, prelude::*};
#[pyclass(name = "MeshBuilder", subclass, frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMeshBuilder;

#[pymethods]
impl PyMeshBuilder {
    pub fn build(pyself: Bound<'_, Self>) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(format!(
            "MeshBuilder.build() not implemented for {}",
            pyself.get_type().name()?
        )))
    }
}
