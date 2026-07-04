use bevy::window::ExitCondition;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ExitCondition)]
#[pyclass(name = "ExitCondition", from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PyExitCondition {
    OnPrimaryClosed,
    OnAllClosed,
    DontExit,
}
