use bevy::{animation::AnimationTargetId, ecs::name::Name};
use pybevy_core::PyComponent;
use pybevy_macros::newtype_storage;
use pyo3::prelude::*;
use uuid::Uuid;

#[newtype_storage(AnimationTargetId, bridge, copy)]
#[pyclass(name = "AnimationTargetId", extends = PyComponent, frozen, eq)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PyAnimationTargetId(pub(crate) AnimationTargetId);

#[pymethods]
impl PyAnimationTargetId {
    #[new]
    pub fn new(py: Python<'_>, uuid: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        if let Ok(value) = uuid.extract::<u128>() {
            return Py::new(
                py,
                Self::from_owned(AnimationTargetId(Uuid::from_u128(value))),
            );
        }

        let int_value: u128 = uuid.getattr("int")?.extract()?;
        Py::new(
            py,
            Self::from_owned(AnimationTargetId(Uuid::from_u128(int_value))),
        )
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

    pub fn as_uuid(&self) -> u128 {
        self.0.0.as_u128()
    }

    pub fn __repr__(&self) -> String {
        format!("AnimationTargetId({})", self.0.0)
    }
}
