use bevy::{prelude::InColorSpace, ui::ConicGradient};
use pybevy_core::{
    ValueStorage,
    repr::{float_repr, object_repr},
};
use pyo3::prelude::*;

use crate::{
    PyInterpolationColorSpace, angular_color_stop::PyAngularColorStop, ui_position::PyUiPosition,
    val::PyVal,
};

#[pyclass(name = "ConicGradient", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyConicGradient {
    pub(crate) inner: ConicGradient,
    read_only: bool,
}

impl From<ConicGradient> for PyConicGradient {
    fn from(gradient: ConicGradient) -> Self {
        PyConicGradient {
            inner: gradient,
            read_only: false,
        }
    }
}

impl PyConicGradient {
    pub(crate) fn read_only_snapshot(inner: ConicGradient) -> Self {
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

impl PartialEq for PyConicGradient {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
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
            read_only: false,
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
            read_only: false,
            inner: self.inner.clone().with_start(start),
        }
    }

    pub fn with_position(&self, position: PyUiPosition) -> PyResult<Self> {
        Ok(PyConicGradient {
            read_only: false,
            inner: self.inner.clone().with_position(position.try_into()?),
        })
    }

    pub fn in_color_space(&self, color_space: PyInterpolationColorSpace) -> Self {
        PyConicGradient {
            read_only: false,
            inner: self.inner.clone().in_color_space(color_space.into()),
        }
    }

    pub fn in_oklaba(&self) -> Self {
        PyConicGradient {
            read_only: false,
            inner: self.inner.clone().in_oklaba(),
        }
    }

    pub fn in_srgb(&self) -> Self {
        PyConicGradient {
            read_only: false,
            inner: self.inner.clone().in_srgb(),
        }
    }

    pub fn in_linear_rgb(&self) -> Self {
        PyConicGradient {
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
    pub fn start(&self) -> f32 {
        self.inner.start
    }

    #[setter]
    pub fn set_start(&mut self, value: f32) -> PyResult<()> {
        self.ensure_writable()?;
        self.inner.start = value;
        Ok(())
    }

    #[getter]
    pub fn position(&self) -> PyUiPosition {
        PyUiPosition::from_borrowed(ValueStorage::read_only_snapshot(self.inner.position))
    }

    #[setter]
    pub fn set_position(&mut self, value: PyUiPosition) -> PyResult<()> {
        self.ensure_writable()?;
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
        self.ensure_writable()?;
        self.inner.stops = value
            .into_iter()
            .map(|stop| stop.to_bevy())
            .collect::<PyResult<_>>()?;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ConicGradient(position={}, start={}, stops={})",
            object_repr(PyUiPosition::from(self.inner.position)),
            float_repr(self.inner.start),
            object_repr(self.stops())
        )
    }
}
