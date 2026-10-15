use bevy::ui::Val2;
use pybevy_core::{FromBorrowedStorage, ValueStorage, repr::object_repr};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyvalue]
#[pyclass(name = "Val2", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyVal2 {
    pub(crate) storage: ValueStorage<Val2>,
}

impl PartialEq for PyVal2 {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl From<Val2> for PyVal2 {
    fn from(value: Val2) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyVal2> for Val2 {
    type Error = PyErr;

    fn try_from(value: PyVal2) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyVal2> for Val2 {
    type Error = PyErr;

    fn try_from(value: &PyVal2) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyVal2 {
    #[new]
    #[pyo3(signature = (x = PyVal::zero(), y = PyVal::zero()))]
    pub fn new(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(Val2::new(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::from_owned(Val2::ZERO)
    }

    #[staticmethod]
    pub fn px(x: f32, y: f32) -> Self {
        Self::from_owned(Val2::px(x, y))
    }

    #[staticmethod]
    pub fn percent(x: f32, y: f32) -> Self {
        Self::from_owned(Val2::percent(x, y))
    }

    #[getter]
    pub fn x(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.x.into())
    }

    #[setter]
    pub fn set_x(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.x = value.into();
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.y.into())
    }

    #[setter]
    pub fn set_y(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.y = value.into();
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let value = self.as_ref()?;
        Ok(format!(
            "Val2(x={}, y={})",
            object_repr(PyVal::from(value.x)),
            object_repr(PyVal::from(value.y))
        ))
    }
}
