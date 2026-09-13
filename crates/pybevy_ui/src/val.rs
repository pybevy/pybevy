use bevy::ui::{Val, percent, px, vh, vmax, vmin, vw};
use pybevy_macros::pyenum;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::ui_rect::PyUiRect;

#[pyenum(Val, empty_tuple, unit_parens)]
#[pyclass(name = "Val", module = "pybevy.ui", frozen, eq, from_py_object)]
#[derive(Clone, Debug)]
pub enum PyVal {
    Auto(),
    #[py_bevy(tuple)]
    Px {
        value: f32,
    },
    #[py_bevy(tuple)]
    Percent {
        value: f32,
    },
    #[py_bevy(tuple)]
    Vw {
        value: f32,
    },
    #[py_bevy(tuple)]
    Vh {
        value: f32,
    },
    #[py_bevy(tuple)]
    VMin {
        value: f32,
    },
    #[py_bevy(tuple)]
    VMax {
        value: f32,
    },
}

impl PartialEq for PyVal {
    fn eq(&self, other: &Self) -> bool {
        self.clone().into_inner() == other.clone().into_inner()
    }
}

impl PyVal {
    pub fn into_inner(self) -> Val {
        self.into()
    }

    pub(crate) const fn px_unchecked(value: f32) -> Self {
        Self::Px { value }
    }

    pub(crate) const fn percent_unchecked(value: f32) -> Self {
        Self::Percent { value }
    }

    pub(crate) const fn zero() -> Self {
        Self::px_unchecked(0.0)
    }
}

/// Bare numbers are PyBevy's documented pixel-value convenience.
pub fn extract_val_from_any(value: &Bound<'_, PyAny>) -> PyResult<Val> {
    if let Ok(value) = value.extract::<PyVal>() {
        return Ok(value.into());
    }
    if let Ok(value) = value.extract::<f32>() {
        return Ok(Val::Px(value));
    }
    Err(PyTypeError::new_err("expected Val or float"))
}

#[pymethods]
impl PyVal {
    #[classattr]
    pub const ZERO: Self = Self::px_unchecked(0.0);

    #[classattr]
    pub const DEFAULT: Self = Self::Auto();

    pub fn left(&self) -> PyUiRect {
        self.clone().into_inner().left().into()
    }

    pub fn right(&self) -> PyUiRect {
        self.clone().into_inner().right().into()
    }

    pub fn top(&self) -> PyUiRect {
        self.clone().into_inner().top().into()
    }

    pub fn bottom(&self) -> PyUiRect {
        self.clone().into_inner().bottom().into()
    }

    pub fn all(&self) -> PyUiRect {
        self.clone().into_inner().all().into()
    }

    pub fn horizontal(&self) -> PyUiRect {
        self.clone().into_inner().horizontal().into()
    }

    pub fn vertical(&self) -> PyUiRect {
        self.clone().into_inner().vertical().into()
    }

    pub fn try_add(&self, val: &Self) -> PyResult<Self> {
        self.clone()
            .into_inner()
            .try_add(val.clone().into_inner())
            .map(Self::from)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    pub fn try_sub(&self, val: &Self) -> PyResult<Self> {
        self.clone()
            .into_inner()
            .try_sub(val.clone().into_inner())
            .map(Self::from)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    pub fn __mul__(&self, rhs: f32) -> Self {
        (self.clone().into_inner() * rhs).into()
    }

    pub fn __truediv__(&self, rhs: f32) -> Self {
        (self.clone().into_inner() / rhs).into()
    }

    pub fn __neg__(&self) -> Self {
        (-self.clone().into_inner()).into()
    }
}

impl Default for PyVal {
    fn default() -> Self {
        Val::default().into()
    }
}

#[pyfunction(name = "px")]
pub fn py_px(value: f32) -> PyVal {
    px(value).into()
}

#[pyfunction(name = "percent")]
pub fn py_percent(value: f32) -> PyVal {
    percent(value).into()
}

#[pyfunction(name = "vw")]
pub fn py_vw(value: f32) -> PyVal {
    vw(value).into()
}

#[pyfunction(name = "vh")]
pub fn py_vh(value: f32) -> PyVal {
    vh(value).into()
}

#[pyfunction(name = "vmin")]
pub fn py_vmin(value: f32) -> PyVal {
    vmin(value).into()
}

#[pyfunction(name = "vmax")]
pub fn py_vmax(value: f32) -> PyVal {
    vmax(value).into()
}
