use bevy::{color::Color, ui::AngularColorStop};
use pybevy_color::color::PyColor;
use pybevy_core::public_error::value_out_of_range;
use pyo3::{exceptions::PyValueError, prelude::*};

/// bevy interpolates with the hint as a 0.0..=1.0 midpoint fraction.
fn check_hint(hint: f32) -> PyResult<f32> {
    if (0.0..=1.0).contains(&hint) {
        Ok(hint)
    } else {
        Err(PyValueError::new_err(value_out_of_range(
            "hint", 0.0, 1.0, hint,
        )))
    }
}

#[pyclass(name = "AngularColorStop", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyAngularColorStop {
    pub(crate) inner: AngularColorStop,
}

impl From<AngularColorStop> for PyAngularColorStop {
    fn from(stop: AngularColorStop) -> Self {
        PyAngularColorStop { inner: stop }
    }
}

impl From<PyAngularColorStop> for AngularColorStop {
    fn from(py_stop: PyAngularColorStop) -> Self {
        py_stop.inner
    }
}

#[pymethods]
impl PyAngularColorStop {
    #[new]
    #[pyo3(signature = (color = None, angle = None, *, hint = 0.5))]
    pub fn new(color: Option<PyColor>, angle: Option<f32>, hint: f32) -> PyResult<Self> {
        let bevy_color = color
            .map(Color::try_from)
            .transpose()?
            .unwrap_or(Color::WHITE);
        Ok(PyAngularColorStop {
            inner: AngularColorStop {
                color: bevy_color,
                angle,
                hint: check_hint(hint)?,
            },
        })
    }

    #[staticmethod]
    pub fn auto(color: PyColor) -> PyResult<Self> {
        let bevy_color = Color::try_from(color)?;
        Ok(PyAngularColorStop {
            inner: AngularColorStop::auto(bevy_color),
        })
    }

    pub fn with_hint(&self, hint: f32) -> PyResult<Self> {
        Ok(PyAngularColorStop {
            inner: self.inner.with_hint(check_hint(hint)?),
        })
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.inner.color, py)
    }

    #[getter]
    pub fn angle(&self) -> Option<f32> {
        self.inner.angle
    }

    #[getter]
    pub fn hint(&self) -> f32 {
        self.inner.hint
    }

    pub fn __repr__(&self) -> String {
        format!(
            "AngularColorStop(color={:?}, angle={:?}, hint={})",
            self.inner.color, self.inner.angle, self.inner.hint
        )
    }
}
