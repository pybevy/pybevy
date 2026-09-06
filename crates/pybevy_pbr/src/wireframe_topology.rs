use bevy::pbr::wireframe::WireframeTopology;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(WireframeTopology, bridge, copy)]
#[pyclass(name = "WireframeTopology", module = "pybevy.pbr", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyWireframeTopology(pub(crate) WireframeTopology);

#[pymethods]
impl PyWireframeTopology {
    #[classattr]
    #[pyo3(name = "Triangles")]
    pub fn triangles(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(WireframeTopology::Triangles))
    }

    #[classattr]
    #[pyo3(name = "Quads")]
    pub fn quads(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(WireframeTopology::Quads))
    }

    pub fn __repr__(&self) -> &'static str {
        match self.0 {
            WireframeTopology::Triangles => "WireframeTopology.Triangles",
            WireframeTopology::Quads => "WireframeTopology.Quads",
        }
    }
}
