use pyo3::{exceptions::PyNotImplementedError, prelude::*};

use crate::mesh::PyMesh;

#[pyclass(
    name = "MeshBuilder",
    module = "pybevy.mesh",
    subclass,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyMeshBuilder;

#[pymethods]
impl PyMeshBuilder {
    pub fn build(pyself: Bound<'_, Self>) -> PyResult<Py<PyMesh>> {
        Err(PyNotImplementedError::new_err(format!(
            "MeshBuilder.build() not implemented for {}",
            pyself.get_type().name()?
        )))
    }
}
