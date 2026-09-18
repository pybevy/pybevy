use bevy::{prelude::InColorSpace, ui::RadialGradient};
use pybevy_core::ValueStorage;
use pyo3::prelude::*;

use crate::{
    PyInterpolationColorSpace, color_stop::PyColorStop,
    radial_gradient_shape::PyRadialGradientShape, ui_position::PyUiPosition, val::PyVal,
};

#[pyclass(name = "RadialGradient", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyRadialGradient {
    pub(crate) inner: RadialGradient,
}

impl From<RadialGradient> for PyRadialGradient {
    fn from(gradient: RadialGradient) -> Self {
        PyRadialGradient { inner: gradient }
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
    #[pyo3(signature = (position = PyUiPosition::center(PyVal::zero(), PyVal::zero()), shape = PyRadialGradientShape::new(), stops = vec![]))]
    pub fn new(
        position: PyUiPosition,
        shape: PyRadialGradientShape,
        stops: Vec<PyColorStop>,
    ) -> PyResult<Self> {
        Ok(PyRadialGradient {
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
            inner: self.inner.clone().in_color_space(color_space.into()),
        }
    }

    pub fn in_oklaba(&self) -> Self {
        PyRadialGradient {
            inner: self.inner.clone().in_oklaba(),
        }
    }

    pub fn in_srgb(&self) -> Self {
        PyRadialGradient {
            inner: self.inner.clone().in_srgb(),
        }
    }

    pub fn in_linear_rgb(&self) -> Self {
        PyRadialGradient {
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
    pub fn position(&self) -> PyUiPosition {
        PyUiPosition::from_borrowed(ValueStorage::read_only_snapshot(self.inner.position))
    }

    #[setter]
    pub fn set_position(&mut self, value: PyUiPosition) -> PyResult<()> {
        self.inner.position = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn shape(&self) -> PyRadialGradientShape {
        self.inner.shape.into()
    }

    #[setter]
    pub fn set_shape(&mut self, value: PyRadialGradientShape) {
        self.inner.shape = value.into();
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
        self.inner.stops = value
            .into_iter()
            .map(|stop| stop.to_bevy())
            .collect::<PyResult<_>>()?;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "RadialGradient(position={:?}, shape={:?}, stops={})",
            self.inner.position,
            self.inner.shape,
            self.inner.stops.len()
        )
    }
}
