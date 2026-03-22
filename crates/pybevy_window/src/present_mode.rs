use bevy::window::PresentMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(PresentMode)]
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
