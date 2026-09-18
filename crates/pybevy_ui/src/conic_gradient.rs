use bevy::{prelude::InColorSpace, ui::ConicGradient};
use pybevy_core::ValueStorage;
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
    pub fn new(position: PyUiPosition, stops: Vec<PyAngularColorStop>) -> PyResult<Self> {
        Ok(PyConicGradient {
            inner: ConicGradient::new(
                position.try_into()?,
                stops
                    .into_iter()
                    .map(|stop| stop.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    pub fn with_start(&self, start: f32) -> Self {
        PyConicGradient {
            inner: self.inner.clone().with_start(start),
        }
    }

    pub fn with_position(&self, position: PyUiPosition) -> PyResult<Self> {
        Ok(PyConicGradient {
            inner: self.inner.clone().with_position(position.try_into()?),
        })
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

    #[setter]
    pub fn set_color_space(&mut self, value: PyInterpolationColorSpace) {
        self.inner.color_space = value.into();
    }

    #[getter]
    pub fn start(&self) -> f32 {
        self.inner.start
    }

    #[setter]
    pub fn set_start(&mut self, value: f32) {
        self.inner.start = value;
    }

    #[getter]
    pub fn position(&self) -> PyUiPosition {
        PyUiPosition::from_borrowed(ValueStorage::read_only_snapshot(self.inner.position))
    }

    #[setter]
    pub fn set_position(&mut self, value: PyUiPosition) -> PyResult<()> {
        self.inner.position = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn stops(&self) -> Vec<PyAngularColorStop> {
        self.inner
            .stops
            .iter()
            .map(|stop| PyAngularColorStop::from_borrowed(ValueStorage::read_only_snapshot(*stop)))
            .collect()
    }

    #[setter]
    pub fn set_stops(&mut self, value: Vec<PyAngularColorStop>) -> PyResult<()> {
        self.inner.stops = value
            .into_iter()
            .map(|stop| stop.to_bevy())
            .collect::<PyResult<_>>()?;
        Ok(())
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
