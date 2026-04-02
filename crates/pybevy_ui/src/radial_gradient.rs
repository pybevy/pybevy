use bevy::{prelude::InColorSpace, ui::RadialGradient};
use pyo3::prelude::*;

use crate::{
    PyInterpolationColorSpace, color_stop::PyColorStop,
    radial_gradient_shape::PyRadialGradientShape, ui_position::PyUiPosition, val::PyVal,
};

#[pyclass(name = "RadialGradient", eq)]
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
    #[pyo3(signature = (position = PyUiPosition::center(PyVal::new(), PyVal::new()), shape = PyRadialGradientShape::new(), stops = vec![]))]
    pub fn new(
        position: PyUiPosition,
        shape: PyRadialGradientShape,
        stops: Vec<PyColorStop>,
    ) -> Self {
        PyRadialGradient {
            inner: RadialGradient::new(
                position.into(),
                shape.into(),
                stops.into_iter().map(|s| s.inner).collect(),
            ),
        }
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

    #[getter]
    pub fn position(&self) -> PyUiPosition {
        self.inner.position.into()
    }

    #[getter]
    pub fn shape(&self) -> PyRadialGradientShape {
        self.inner.shape.into()
    }

    #[getter]
    pub fn stops(&self) -> Vec<PyColorStop> {
        self.inner.stops.iter().cloned().map(|s| s.into()).collect()
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
