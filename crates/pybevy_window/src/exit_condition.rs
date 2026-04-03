use bevy::window::ExitCondition;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ExitCondition)]
#[pyclass(name = "ExitCondition")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PyExitCondition {
    OnPrimaryClosed,
    OnAllClosed,
    DontExit,
}
