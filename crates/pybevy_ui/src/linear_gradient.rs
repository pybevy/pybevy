use bevy::{prelude::InColorSpace, ui::LinearGradient};
use pybevy_core::{
    ValueStorage,
    repr::{float_repr, object_repr},
};
use pyo3::prelude::*;

use crate::{PyInterpolationColorSpace, color_stop::PyColorStop};

#[pyclass(name = "LinearGradient", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyLinearGradient {
    pub(crate) inner: LinearGradient,
    read_only: bool,
}

impl From<LinearGradient> for PyLinearGradient {
    fn from(gradient: LinearGradient) -> Self {
        PyLinearGradient {
            inner: gradient,
            read_only: false,
        }
    }
}

impl PyLinearGradient {
    pub(crate) fn read_only_snapshot(inner: LinearGradient) -> Self {
        Self {
            inner,
            read_only: true,
        }
    }

    fn ensure_writable(&self) -> PyResult<()> {
        if self.read_only {
            return Err(pybevy_core::StorageError::OwnedFieldReadOnly.into());
        }
        Ok(())
    }
}

impl PartialEq for PyLinearGradient {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
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
    pub fn new(angle: f32, stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::new(
                angle,
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn degrees(degrees: f32, stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::degrees(
                degrees,
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn to_top(stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::to_top(
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn to_bottom(stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::to_bottom(
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn to_left(stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::to_left(
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn to_right(stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::to_right(
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn to_top_left(stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::to_top_left(
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn to_top_right(stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::to_top_right(
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn to_bottom_left(stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::to_bottom_left(
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    #[staticmethod]
    pub fn to_bottom_right(stops: Vec<PyColorStop>) -> PyResult<Self> {
        Ok(PyLinearGradient {
            read_only: false,
            inner: LinearGradient::to_bottom_right(
                stops
                    .into_iter()
                    .map(|s| s.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    pub fn in_color_space(&self, color_space: PyInterpolationColorSpace) -> Self {
        PyLinearGradient {
            read_only: false,
            inner: self.inner.clone().in_color_space(color_space.into()),
        }
    }

    pub fn in_oklaba(&self) -> Self {
        PyLinearGradient {
            read_only: false,
            inner: self.inner.clone().in_oklaba(),
        }
    }

    pub fn in_srgb(&self) -> Self {
        PyLinearGradient {
            read_only: false,
            inner: self.inner.clone().in_srgb(),
        }
    }

    pub fn in_linear_rgb(&self) -> Self {
        PyLinearGradient {
            read_only: false,
            inner: self.inner.clone().in_linear_rgb(),
        }
    }

    #[getter]
    pub fn color_space(&self) -> PyInterpolationColorSpace {
        self.inner.color_space.into()
    }

    #[setter]
    pub fn set_color_space(&mut self, value: PyInterpolationColorSpace) -> PyResult<()> {
        self.ensure_writable()?;
        self.inner.color_space = value.into();
        Ok(())
    }

    #[getter]
    pub fn angle(&self) -> f32 {
        self.inner.angle
    }

    #[setter]
    pub fn set_angle(&mut self, value: f32) -> PyResult<()> {
        self.ensure_writable()?;
        self.inner.angle = value;
        Ok(())
    }

    #[getter]
    pub fn stops(&self) -> Vec<PyColorStop> {
        self.inner
            .stops
            .iter()
            .map(|stop| PyColorStop::from_borrowed(ValueStorage::read_only_snapshot(*stop)))
            .collect()
    }

    #[setter]
    pub fn set_stops(&mut self, value: Vec<PyColorStop>) -> PyResult<()> {
        self.ensure_writable()?;
        self.inner.stops = value
            .into_iter()
            .map(|stop| stop.to_bevy())
            .collect::<PyResult<_>>()?;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "LinearGradient(angle={}, stops={})",
            float_repr(self.inner.angle),
            object_repr(self.stops())
        )
    }
}
