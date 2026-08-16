use bevy::ui::Val;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::ui_rect::PyUiRect;

#[pyclass(name = "Val", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyVal {
    pub(crate) inner: Val,
}

impl PyVal {
    pub fn into_inner(self) -> Val {
        self.inner
    }

    pub(crate) const fn px_unchecked(value: f32) -> Self {
        Self {
            inner: Val::Px(value),
        }
    }

    pub(crate) const fn percent_unchecked(value: f32) -> Self {
        Self {
            inner: Val::Percent(value),
        }
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

/// Extract a [`Val`] from its wrapper or a pixel value.
///
/// Bare numbers mirror PyBevy's documented `float -> Val::Px` adaptation.
pub fn extract_val_from_any(value: &Bound<'_, PyAny>) -> PyResult<Val> {
    if let Ok(value) = value.extract::<PyVal>() {
        return Ok(value.into());
    }
    if let Ok(value) = value.extract::<f32>() {
        return Ok(Val::Px(validate_finite_val("pixel", value)?));
    }
    Err(PyTypeError::new_err("expected Val or float"))
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
    pub fn px(value: f32) -> PyResult<Self> {
        Ok(Self::px_unchecked(validate_finite_val("px", value)?))
    }

    #[staticmethod]
    pub fn percent(value: f32) -> PyResult<Self> {
        Ok(Self::percent_unchecked(validate_finite_val(
            "percent", value,
        )?))
    }

    #[staticmethod]
    pub fn auto() -> Self {
        PyVal { inner: Val::Auto }
    }

    #[staticmethod]
    pub fn vw(value: f32) -> PyResult<Self> {
        Ok(PyVal {
            inner: Val::Vw(validate_finite_val("vw", value)?),
        })
    }

    #[staticmethod]
    pub fn vh(value: f32) -> PyResult<Self> {
        Ok(PyVal {
            inner: Val::Vh(validate_finite_val("vh", value)?),
        })
    }

    #[staticmethod]
    pub fn vmin(value: f32) -> PyResult<Self> {
        Ok(PyVal {
            inner: Val::VMin(validate_finite_val("vmin", value)?),
        })
    }

    #[staticmethod]
    pub fn vmax(value: f32) -> PyResult<Self> {
        Ok(PyVal {
            inner: Val::VMax(validate_finite_val("vmax", value)?),
        })
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
        self.inner == other.inner
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

pub(crate) fn validate_finite_val(unit: &str, value: f32) -> PyResult<f32> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(PyValueError::new_err(format!(
            "Val.{unit} value must be finite (got {value})"
        )))
    }
}

impl Default for PyVal {
    fn default() -> Self {
        Self::new()
    }
}
