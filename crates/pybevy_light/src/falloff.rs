use bevy::light::atmosphere::Falloff;
use pyo3::prelude::*;

#[pyclass(name = "Falloff", frozen)]
#[derive(Clone)]
pub struct PyFalloff(pub(crate) Falloff);

impl From<Falloff> for PyFalloff {
    fn from(val: Falloff) -> Self {
        PyFalloff(val)
    }
}

impl From<PyFalloff> for Falloff {
    fn from(val: PyFalloff) -> Self {
        val.0
    }
}

#[pymethods]
impl PyFalloff {
    #[staticmethod]
    pub fn linear() -> Self {
        PyFalloff(Falloff::Linear)
    }

    #[staticmethod]
    pub fn exponential(scale: f32) -> Self {
        PyFalloff(Falloff::Exponential { scale })
    }

    #[staticmethod]
    pub fn tent(center: f32, width: f32) -> Self {
        PyFalloff(Falloff::Tent { center, width })
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            Falloff::Linear => "Falloff.linear()".to_string(),
            Falloff::Exponential { scale } => format!("Falloff.exponential({})", scale),
            Falloff::Tent { center, width } => format!("Falloff.tent({}, {})", center, width),
            Falloff::Curve(_) => "Falloff.curve(...)".to_string(),
        }
    }
}
