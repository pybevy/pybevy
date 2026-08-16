use bevy::ui::Gradient;
use pybevy_color::color::PyColor;
use pyo3::prelude::*;

use crate::{
    conic_gradient::PyConicGradient, linear_gradient::PyLinearGradient,
    radial_gradient::PyRadialGradient,
};

#[pyclass(name = "Gradient", eq, frozen, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyGradient {
    pub inner: Gradient,
}

impl From<Gradient> for PyGradient {
    fn from(gradient: Gradient) -> Self {
        PyGradient { inner: gradient }
    }
}

impl From<PyGradient> for Gradient {
    fn from(py_gradient: PyGradient) -> Self {
        py_gradient.inner
    }
}

#[pymethods]
impl PyGradient {
    #[staticmethod]
    pub fn linear(gradient: PyLinearGradient) -> Self {
        PyGradient {
            inner: Gradient::Linear(gradient.into()),
        }
    }

    #[staticmethod]
    pub fn radial(gradient: PyRadialGradient) -> Self {
        PyGradient {
            inner: Gradient::Radial(gradient.into()),
        }
    }

    #[staticmethod]
    pub fn conic(gradient: PyConicGradient) -> Self {
        PyGradient {
            inner: Gradient::Conic(gradient.into()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn get_single(&self, py: Python) -> PyResult<Option<Py<PyColor>>> {
        match self.inner.get_single() {
            Some(c) => Ok(Some(PyColor::from_color(c, py)?)),
            None => Ok(None),
        }
    }

    pub fn __repr__(&self) -> String {
        match &self.inner {
            Gradient::Linear(g) => format!("Gradient.linear({:?})", g),
            Gradient::Radial(g) => format!("Gradient.radial({:?})", g),
            Gradient::Conic(g) => format!("Gradient.conic({:?})", g),
        }
    }
}
