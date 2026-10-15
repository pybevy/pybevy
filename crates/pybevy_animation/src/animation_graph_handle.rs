use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle};
use pybevy_core::{HandleArg, PyComponent, PyHandle};
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
    #[pyo3(signature = (handle = HandleArg::default()))]
    pub fn new(handle: HandleArg) -> PyResult<PyClassInitializer<Self>> {
        Ok((
            Self(handle.into_py_handle::<AnimationGraph>()?),
            PyComponent,
        )
            .into())
    }

    #[getter]
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
