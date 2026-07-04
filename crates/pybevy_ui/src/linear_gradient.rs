use bevy::{prelude::InColorSpace, ui::LinearGradient};
use pyo3::prelude::*;

use crate::{PyInterpolationColorSpace, color_stop::PyColorStop};

#[pyclass(name = "LinearGradient", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyLinearGradient {
    pub(crate) inner: LinearGradient,
}

impl From<LinearGradient> for PyLinearGradient {
    fn from(gradient: LinearGradient) -> Self {
        PyLinearGradient { inner: gradient }
    }
}

impl From<PyLinearGradient> for LinearGradient {
    fn from(py_gradient: PyLinearGradient) -> Self {
        py_gradient.inner
    }
}

#[pymethods]
impl PyLinearGradient {
    #[new]
    #[pyo3(signature = (angle, stops))]
    pub fn new(angle: f32, stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::new(angle, stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn degrees(degrees: f32, stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::degrees(degrees, stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn to_top(stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::to_top(stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn to_bottom(stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::to_bottom(stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn to_left(stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::to_left(stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn to_right(stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::to_right(stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn to_top_left(stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::to_top_left(stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn to_top_right(stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::to_top_right(stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn to_bottom_left(stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::to_bottom_left(stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    #[staticmethod]
    pub fn to_bottom_right(stops: Vec<PyColorStop>) -> Self {
        PyLinearGradient {
            inner: LinearGradient::to_bottom_right(stops.into_iter().map(|s| s.inner).collect()),
        }
    }

    pub fn in_color_space(&self, color_space: PyInterpolationColorSpace) -> Self {
        PyLinearGradient {
            inner: self.inner.clone().in_color_space(color_space.into()),
        }
    }

    pub fn in_oklaba(&self) -> Self {
        PyLinearGradient {
            inner: self.inner.clone().in_oklaba(),
        }
    }

    pub fn in_srgb(&self) -> Self {
        PyLinearGradient {
            inner: self.inner.clone().in_srgb(),
        }
    }

    pub fn in_linear_rgb(&self) -> Self {
        PyLinearGradient {
            inner: self.inner.clone().in_linear_rgb(),
        }
    }

    #[getter]
    pub fn color_space(&self) -> PyInterpolationColorSpace {
        self.inner.color_space.into()
    }

    #[getter]
    pub fn angle(&self) -> f32 {
        self.inner.angle
    }

    #[getter]
    pub fn stops(&self) -> Vec<PyColorStop> {
        self.inner.stops.iter().cloned().map(|s| s.into()).collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "LinearGradient(angle={}, stops={:?})",
            self.inner.angle,
            self.inner.stops.len()
        )
    }
}
