use bevy::post_process::dof::DepthOfFieldMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(DepthOfFieldMode)]
#[pyclass(
    name = "DepthOfFieldMode",
    module = "pybevy.post_process",
    eq,
    from_py_object,
    frozen
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyDepthOfFieldMode {
    Bokeh,
    Gaussian,
}
