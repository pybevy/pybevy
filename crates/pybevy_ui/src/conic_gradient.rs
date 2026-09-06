use bevy::{prelude::InColorSpace, ui::ConicGradient};
use pyo3::prelude::*;

use crate::{
    PyInterpolationColorSpace, angular_color_stop::PyAngularColorStop, ui_position::PyUiPosition,
    val::PyVal,
};

#[pyclass(name = "ConicGradient", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyConicGradient {
    pub(crate) inner: ConicGradient,
}

impl From<ConicGradient> for PyConicGradient {
    fn from(gradient: ConicGradient) -> Self {
        PyConicGradient { inner: gradient }
    }
}

impl From<PyConicGradient> for ConicGradient {
    fn from(py_gradient: PyConicGradient) -> Self {
        py_gradient.inner
    }
}

#[pymethods]
impl PyConicGradient {
    #[new]
    #[pyo3(signature = (position = PyUiPosition::center(PyVal::zero(), PyVal::zero()), stops = vec![]))]
    pub fn new(position: PyUiPosition, stops: Vec<PyAngularColorStop>) -> Self {
        PyConicGradient {
            inner: ConicGradient::new(
                position.into(),
                stops.into_iter().map(|s| s.inner).collect(),
            ),
        }
    }

    pub fn with_start(&self, start: f32) -> Self {
        PyConicGradient {
            inner: self.inner.clone().with_start(start),
        }
    }

    pub fn with_position(&self, position: PyUiPosition) -> Self {
        PyConicGradient {
            inner: self.inner.clone().with_position(position.into()),
        }
    }

    pub fn in_color_space(&self, color_space: PyInterpolationColorSpace) -> Self {
        PyConicGradient {
            inner: self.inner.clone().in_color_space(color_space.into()),
        }
    }

    pub fn in_oklaba(&self) -> Self {
        PyConicGradient {
            inner: self.inner.clone().in_oklaba(),
        }
    }

    pub fn in_srgb(&self) -> Self {
        PyConicGradient {
            inner: self.inner.clone().in_srgb(),
        }
    }

    pub fn in_linear_rgb(&self) -> Self {
        PyConicGradient {
            inner: self.inner.clone().in_linear_rgb(),
        }
    }

    #[getter]
    pub fn color_space(&self) -> PyInterpolationColorSpace {
        self.inner.color_space.into()
    }

    #[getter]
    pub fn start(&self) -> f32 {
        self.inner.start
    }

    #[getter]
    pub fn position(&self) -> PyUiPosition {
        self.inner.position.into()
    }

    #[getter]
    pub fn stops(&self) -> Vec<PyAngularColorStop> {
        self.inner.stops.iter().cloned().map(|s| s.into()).collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ConicGradient(position={:?}, start={}, stops={})",
            self.inner.position,
            self.inner.start,
            self.inner.stops.len()
        )
    }
}
