use bevy::window::PresentMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(PresentMode)]
#[pyclass(name = "PresentMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyPresentMode {
    AutoVsync,
    AutoNoVsync,
    Fifo,
    FifoRelaxed,
    Immediate,
    Mailbox,
}
