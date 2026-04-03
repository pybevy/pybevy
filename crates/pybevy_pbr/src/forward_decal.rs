use bevy::pbr::decal::ForwardDecal;
use pybevy_core::PyComponent;
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(ForwardDecal, unit, bridge)]
#[pyclass(name = "ForwardDecal", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyForwardDecal;

impl From<ForwardDecal> for PyForwardDecal {
    fn from(_: ForwardDecal) -> Self {
        PyForwardDecal
    }
}

impl From<PyForwardDecal> for ForwardDecal {
    fn from(_: PyForwardDecal) -> Self {
        ForwardDecal
    }
}

impl TryFrom<&ForwardDecal> for PyForwardDecal {
    type Error = PyErr;
    fn try_from(_: &ForwardDecal) -> PyResult<Self> {
        Ok(PyForwardDecal)
    }
}

#[pymethods]
impl PyForwardDecal {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyForwardDecal, PyComponent)
    }

    fn __repr__(&self) -> &'static str {
        "ForwardDecal"
    }
}
