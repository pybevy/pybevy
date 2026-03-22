use bevy::input::touch::TouchPhase;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(TouchPhase, from_only)]
#[pyclass(name = "TouchPhase", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyTouchPhase {
    Started,
    Moved,
    Ended,
    Canceled,
}

#[pymethods]
impl PyTouchPhase {
    pub fn __repr__(&self) -> String {
        format!("TouchPhase.{:?}", self)
    }
}
