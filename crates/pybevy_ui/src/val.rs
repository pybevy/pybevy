use bevy::ui::Val;
use pyo3::prelude::*;

use crate::ui_rect::PyUiRect;

#[pyclass(name = "Val")]
#[derive(Clone, Debug)]
pub struct PyVal {
    pub(crate) inner: Val,
}

impl PyVal {
    pub fn into_inner(self) -> Val {
        self.inner
    }
}

impl From<Val> for PyVal {
    fn from(val: Val) -> Self {
        PyVal { inner: val }
    }
}

impl From<PyVal> for Val {
    fn from(py_val: PyVal) -> Self {
        py_val.inner
    }
}

#[pymethods]
impl PyVal {
    #[new]
    pub fn new() -> Self {
        PyVal {
            inner: Val::default(),
        }
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        PyVal { inner: Val::ZERO }
    }

    #[staticmethod]
    pub fn px(value: f32) -> Self {
        PyVal {
            inner: Val::Px(value),
        }
    }

    #[staticmethod]
    pub fn percent(value: f32) -> Self {
        PyVal {
            inner: Val::Percent(value),
        }
    }

    #[staticmethod]
    pub fn auto() -> Self {
        PyVal { inner: Val::Auto }
    }

    #[staticmethod]
    pub fn vw(value: f32) -> Self {
        PyVal {
            inner: Val::Vw(value),
        }
    }

    #[staticmethod]
    pub fn vh(value: f32) -> Self {
        PyVal {
            inner: Val::Vh(value),
        }
    }

    #[staticmethod]
    pub fn vmin(value: f32) -> Self {
        PyVal {
            inner: Val::VMin(value),
        }
    }

    #[staticmethod]
    pub fn vmax(value: f32) -> Self {
        PyVal {
            inner: Val::VMax(value),
        }
    }

    pub fn left(&self) -> PyUiRect {
        PyUiRect::from(self.inner.left())
    }

    pub fn right(&self) -> PyUiRect {
        PyUiRect::from(self.inner.right())
    }

    pub fn top(&self) -> PyUiRect {
        PyUiRect::from(self.inner.top())
    }

    pub fn bottom(&self) -> PyUiRect {
        PyUiRect::from(self.inner.bottom())
    }

    pub fn all(&self) -> PyUiRect {
        PyUiRect::from(self.inner.all())
    }

    pub fn horizontal(&self) -> PyUiRect {
        PyUiRect::from(self.inner.horizontal())
    }

    pub fn vertical(&self) -> PyUiRect {
        PyUiRect::from(self.inner.vertical())
    }

    pub fn __repr__(&self) -> String {
        match self.inner {
            Val::Px(v) => format!("Val.px({})", v),
            Val::Percent(v) => format!("Val.percent({})", v),
            Val::Auto => "Val.auto()".to_string(),
            Val::Vw(v) => format!("Val.vw({})", v),
            Val::Vh(v) => format!("Val.vh({})", v),
            Val::VMin(v) => format!("Val.vmin({})", v),
            Val::VMax(v) => format!("Val.vmax({})", v),
        }
    }

    pub fn __eq__(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (Val::Px(a), Val::Px(b)) => (a - b).abs() < f32::EPSILON,
            (Val::Percent(a), Val::Percent(b)) => (a - b).abs() < f32::EPSILON,
            (Val::Auto, Val::Auto) => true,
            (Val::Vw(a), Val::Vw(b)) => (a - b).abs() < f32::EPSILON,
            (Val::Vh(a), Val::Vh(b)) => (a - b).abs() < f32::EPSILON,
            (Val::VMin(a), Val::VMin(b)) => (a - b).abs() < f32::EPSILON,
            (Val::VMax(a), Val::VMax(b)) => (a - b).abs() < f32::EPSILON,
            _ => false,
        }
    }

    pub fn __hash__(&self) -> u64 {
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };

        let mut hasher = DefaultHasher::new();
        match self.inner {
            Val::Px(v) => {
                0u8.hash(&mut hasher);
                v.to_bits().hash(&mut hasher);
            }
            Val::Percent(v) => {
                1u8.hash(&mut hasher);
                v.to_bits().hash(&mut hasher);
            }
            Val::Auto => {
                2u8.hash(&mut hasher);
            }
            Val::Vw(v) => {
                3u8.hash(&mut hasher);
                v.to_bits().hash(&mut hasher);
            }
            Val::Vh(v) => {
                4u8.hash(&mut hasher);
                v.to_bits().hash(&mut hasher);
            }
            Val::VMin(v) => {
                5u8.hash(&mut hasher);
                v.to_bits().hash(&mut hasher);
            }
            Val::VMax(v) => {
                6u8.hash(&mut hasher);
                v.to_bits().hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    #[getter]
    pub fn px_value(&self) -> Option<f32> {
        match self.inner {
            Val::Px(v) => Some(v),
            _ => None,
        }
    }

    #[getter]
    pub fn percent_value(&self) -> Option<f32> {
        match self.inner {
            Val::Percent(v) => Some(v),
            _ => None,
        }
    }

    #[getter]
    pub fn is_auto(&self) -> bool {
        matches!(self.inner, Val::Auto)
    }
}

impl Default for PyVal {
    fn default() -> Self {
        Self::new()
    }
}
