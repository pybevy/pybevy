use bevy::{color::Color, ui::AngularColorStop};
use pybevy_color::color::PyColor;
use pyo3::prelude::*;

#[pyclass(name = "AngularColorStop", eq)]
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
    pub fn new(color: Option<PyColor>, angle: Option<f32>, hint: f32) -> Self {
        let bevy_color: Color = color.map(|c| c.into()).unwrap_or(Color::WHITE);
        PyAngularColorStop {
            inner: AngularColorStop {
                color: bevy_color,
                angle,
                hint,
            },
        }
    }

    #[staticmethod]
    pub fn auto(color: PyColor) -> Self {
        let bevy_color: Color = color.into();
        PyAngularColorStop {
            inner: AngularColorStop::auto(bevy_color),
        }
    }

    pub fn with_hint(&self, hint: f32) -> Self {
        PyAngularColorStop {
            inner: self.inner.with_hint(hint),
        }
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
