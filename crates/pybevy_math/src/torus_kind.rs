use bevy::math::primitives::TorusKind;
use pyo3::prelude::*;

#[pyclass(name = "TorusKind", module = "pybevy.math", eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyTorusKind {
    Ring,
    Horn,
    Spindle,
    Invalid,
}

impl From<TorusKind> for PyTorusKind {
    fn from(kind: TorusKind) -> Self {
        match kind {
            TorusKind::Ring => PyTorusKind::Ring,
            TorusKind::Horn => PyTorusKind::Horn,
            TorusKind::Spindle => PyTorusKind::Spindle,
            TorusKind::Invalid => PyTorusKind::Invalid,
        }
    }
}

#[pymethods]
impl PyTorusKind {
    fn __repr__(&self) -> String {
        match self {
            PyTorusKind::Ring => "TorusKind.Ring".to_string(),
            PyTorusKind::Horn => "TorusKind.Horn".to_string(),
            PyTorusKind::Spindle => "TorusKind.Spindle".to_string(),
            PyTorusKind::Invalid => "TorusKind.Invalid".to_string(),
        }
    }
}
