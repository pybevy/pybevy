use bevy::ui::Val2;
use pyo3::prelude::*;

use crate::val::{PyVal, validate_finite_val};

#[pyclass(name = "Val2", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PyVal2 {
    pub(crate) inner: Val2,
}

impl From<Val2> for PyVal2 {
    fn from(value: Val2) -> Self {
        PyVal2 { inner: value }
    }
}

impl From<PyVal2> for Val2 {
    fn from(value: PyVal2) -> Self {
        value.inner
    }
}

#[pymethods]
impl PyVal2 {
    #[new]
    #[pyo3(signature = (x = PyVal::zero(), y = PyVal::zero()))]
    pub fn new(x: PyVal, y: PyVal) -> Self {
        PyVal2 {
            inner: Val2::new(x.into(), y.into()),
        }
    }

    #[classattr]
    pub const ZERO: PyVal2 = PyVal2 { inner: Val2::ZERO };

    #[staticmethod]
    pub fn px(x: f32, y: f32) -> PyResult<Self> {
        Ok(PyVal2 {
            inner: Val2::px(validate_finite_val("px", x)?, validate_finite_val("px", y)?),
        })
    }

    #[staticmethod]
    pub fn percent(x: f32, y: f32) -> PyResult<Self> {
        Ok(PyVal2 {
            inner: Val2::percent(
                validate_finite_val("percent", x)?,
                validate_finite_val("percent", y)?,
            ),
        })
    }

    #[getter]
    pub fn x(&self) -> PyVal {
        self.inner.x.into()
    }

    #[getter]
    pub fn y(&self) -> PyVal {
        self.inner.y.into()
    }

    fn __repr__(&self) -> String {
        format!("Val2(x={:?}, y={:?})", self.inner.x, self.inner.y)
    }
}
