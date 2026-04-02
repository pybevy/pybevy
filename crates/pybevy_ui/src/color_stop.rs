use bevy::{color::Color, ui::ColorStop};
use pybevy_color::color::PyColor;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyclass(name = "ColorStop", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyColorStop {
    pub(crate) inner: ColorStop,
}

impl From<ColorStop> for PyColorStop {
    fn from(stop: ColorStop) -> Self {
        PyColorStop { inner: stop }
    }
}

impl From<PyColorStop> for ColorStop {
    fn from(py_stop: PyColorStop) -> Self {
        py_stop.inner
    }
}

#[pymethods]
impl PyColorStop {
    #[new]
    #[pyo3(signature = (color = None, point = PyVal::zero(), *, hint = 0.5))]
    pub fn new(color: Option<PyColor>, point: PyVal, hint: f32) -> Self {
        let bevy_color: Color = color.map(|c| c.into()).unwrap_or(Color::NONE);
        PyColorStop {
            inner: ColorStop {
                color: bevy_color,
                point: point.into(),
                hint,
            },
        }
    }

    #[staticmethod]
    pub fn auto(color: PyColor) -> Self {
        let bevy_color: Color = color.into();
        PyColorStop {
            inner: ColorStop::auto(bevy_color),
        }
    }

    #[staticmethod]
    pub fn px(color: PyColor, px: f32) -> Self {
        let bevy_color: Color = color.into();
        PyColorStop {
            inner: ColorStop::px(bevy_color, px),
        }
    }

    #[staticmethod]
    pub fn percent(color: PyColor, percent: f32) -> Self {
        let bevy_color: Color = color.into();
        PyColorStop {
            inner: ColorStop::percent(bevy_color, percent),
        }
    }

    pub fn with_hint(&self, hint: f32) -> Self {
        PyColorStop {
            inner: self.inner.with_hint(hint),
        }
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.inner.color, py)
    }

    #[getter]
    pub fn point(&self) -> PyVal {
        self.inner.point.into()
    }

    #[getter]
    pub fn hint(&self) -> f32 {
        self.inner.hint
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ColorStop(color={:?}, point={:?}, hint={})",
            self.inner.color, self.inner.point, self.inner.hint
        )
    }
}
