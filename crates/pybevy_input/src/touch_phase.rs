use bevy::input::touch::TouchPhase;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(TouchPhase)]
#[pyclass(name = "TouchPhase", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyTouchPhase {
    Started,
    Moved,
    Ended,
    Canceled,
}
