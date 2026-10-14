use bevy::{prelude::InColorSpace, ui::RadialGradient};
use pybevy_core::{ValueStorage, repr::object_repr};
use pyo3::prelude::*;

use crate::{
    PyInterpolationColorSpace, color_stop::PyColorStop,
    radial_gradient_shape::PyRadialGradientShape, ui_position::PyUiPosition, val::PyVal,
};

#[pyclass(name = "RadialGradient", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyRadialGradient {
    pub(crate) inner: RadialGradient,
    read_only: bool,
}

impl From<RadialGradient> for PyRadialGradient {
    fn from(gradient: RadialGradient) -> Self {
        PyRadialGradient {
            inner: gradient,
            read_only: false,
        }
    }
}

impl PyRadialGradient {
    pub(crate) fn read_only_snapshot(inner: RadialGradient) -> Self {
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

impl PartialEq for PyRadialGradient {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl From<PyRadialGradient> for RadialGradient {
    fn from(py_gradient: PyRadialGradient) -> Self {
        py_gradient.inner
    }
}

#[pymethods]
impl PyRadialGradient {
    #[new]
    #[pyo3(signature = (position = PyUiPosition::center(PyVal::zero(), PyVal::zero()), shape = PyRadialGradientShape::default(), stops = vec![]))]
    pub fn new(
        position: PyUiPosition,
        shape: PyRadialGradientShape,
        stops: Vec<PyColorStop>,
    ) -> PyResult<Self> {
        Ok(PyRadialGradient {
            read_only: false,
            inner: RadialGradient::new(
                position.try_into()?,
                shape.into(),
                stops
                    .into_iter()
                    .map(|stop| stop.to_bevy())
                    .collect::<PyResult<_>>()?,
            ),
        })
    }

    pub fn in_color_space(&self, color_space: PyInterpolationColorSpace) -> Self {
        PyRadialGradient {
            read_only: false,
            inner: self.inner.clone().in_color_space(color_space.into()),
        }
    }

    pub fn in_oklaba(&self) -> Self {
        PyRadialGradient {
            read_only: false,
            inner: self.inner.clone().in_oklaba(),
        }
    }

    pub fn in_srgb(&self) -> Self {
        PyRadialGradient {
            read_only: false,
            inner: self.inner.clone().in_srgb(),
        }
    }

    pub fn in_linear_rgb(&self) -> Self {
        PyRadialGradient {
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
    pub fn shape(&self) -> PyRadialGradientShape {
        self.inner.shape.into()
    }

    #[setter]
    pub fn set_shape(&mut self, value: PyRadialGradientShape) -> PyResult<()> {
        self.ensure_writable()?;
        self.inner.shape = value.into();
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
            "RadialGradient(position={}, shape={}, stops={})",
            object_repr(PyUiPosition::from(self.inner.position)),
            object_repr(PyRadialGradientShape::from(self.inner.shape)),
            object_repr(self.stops())
        )
    }
}
