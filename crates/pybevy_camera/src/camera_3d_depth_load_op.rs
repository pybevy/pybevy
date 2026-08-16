use bevy::camera::Camera3dDepthLoadOp;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(Camera3dDepthLoadOp, empty_tuple, unit_parens)]
#[pyclass(name = "Camera3dDepthLoadOp", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyCamera3dDepthLoadOp {
    #[py_bevy(tuple)]
    Clear {
        value: f32,
    },
    Load(),
}
