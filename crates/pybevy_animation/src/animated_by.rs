use bevy::animation::AnimatedBy;
use pybevy_core::{ComponentStorage, PyComponent, PyEntity};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(AnimatedBy, bridge)]
#[pyclass(name = "AnimatedBy", extends = PyComponent)]
#[derive(Debug)]
pub struct PyAnimatedBy {
    pub(crate) storage: ComponentStorage<AnimatedBy>,
}

#[pymethods]
impl PyAnimatedBy {
    #[new]
    pub fn new(entity: &PyEntity) -> (Self, PyComponent) {
        Self::from_owned(AnimatedBy(entity.0))
    }

    #[getter]
    pub fn entity(&self) -> PyResult<PyEntity> {
        Ok(PyEntity::from(self.as_ref()?.0))
    }
}
