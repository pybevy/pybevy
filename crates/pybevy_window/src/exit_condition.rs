use bevy::window::ExitCondition;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ExitCondition)]
#[pyclass(name = "ExitCondition")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PyExitCondition {
    OnPrimaryClosed,
    OnAllClosed,
    DontExit,
}
