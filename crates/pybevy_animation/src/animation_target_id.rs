use bevy::{animation::AnimationTargetId, ecs::name::Name};
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;
use uuid::Uuid;

#[pywrap(AnimationTargetId, bridge, copy)]
#[pyclass(from_py_object, name = "AnimationTargetId", extends = PyComponent, frozen, eq, hash)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyAnimationTargetId(pub(crate) AnimationTargetId);

#[pymethods]
impl PyAnimationTargetId {
    #[new]
    pub fn new(py: Python<'_>, uuid: Uuid) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(AnimationTargetId(uuid)))
    }

    #[staticmethod]
    pub fn from_name(py: Python<'_>, name: String) -> PyResult<Py<Self>> {
        let bevy_name = Name::new(name);
        Py::new(
            py,
            Self::from_owned(AnimationTargetId::from_name(&bevy_name)),
        )
    }

    #[staticmethod]
    pub fn from_names(py: Python<'_>, names: Vec<String>) -> PyResult<Py<Self>> {
        let bevy_names: Vec<Name> = names.iter().map(|s| Name::new(s.clone())).collect();
        Py::new(
            py,
            Self::from_owned(AnimationTargetId::from_names(bevy_names.iter())),
        )
    }

    #[getter]
    pub fn value(&self) -> Uuid {
        self.0.0
    }

    pub fn __repr__(&self) -> String {
        format!("AnimationTargetId({})", self.0.0)
    }
}
