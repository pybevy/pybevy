use bevy::input::touch::TouchPhase;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(TouchPhase)]
#[pyclass(name = "TouchPhase", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyTouchPhase {
    Started,
    Moved,
    Ended,
    Canceled,
}
