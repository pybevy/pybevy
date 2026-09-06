use bevy::animation::graph::AnimationGraphHandle;
use pybevy_core::{PyComponent, PyHandle};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(AnimationGraphHandle)]
#[pyclass(name = "AnimationGraphHandle", module = "pybevy.animation", extends = PyComponent, eq, frozen, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAnimationGraphHandle(pub(crate) PyHandle);

impl TryFrom<PyAnimationGraphHandle> for AnimationGraphHandle {
    type Error = PyErr;

    fn try_from(value: PyAnimationGraphHandle) -> Result<Self, Self::Error> {
        Ok(AnimationGraphHandle(value.0.try_into()?))
    }
}

impl TryFrom<&PyAnimationGraphHandle> for AnimationGraphHandle {
    type Error = PyErr;

    fn try_from(value: &PyAnimationGraphHandle) -> Result<Self, Self::Error> {
        Ok(AnimationGraphHandle((&value.0).try_into()?))
    }
}

impl From<&AnimationGraphHandle> for PyAnimationGraphHandle {
    fn from(value: &AnimationGraphHandle) -> Self {
        PyAnimationGraphHandle((&value.0).into())
    }
}

#[pymethods]
impl PyAnimationGraphHandle {
    #[new]
    pub fn new(value: PyHandle) -> PyResult<PyClassInitializer<Self>> {
        Ok((Self(value), PyComponent).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
