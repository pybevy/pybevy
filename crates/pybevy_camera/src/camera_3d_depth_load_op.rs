use bevy::camera::Camera3dDepthLoadOp;
use pyo3::prelude::*;

#[pyclass(name = "Camera3dDepthLoadOp", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyCamera3dDepthLoadOp {
    Clear(f32),
    Load(),
}

impl From<Camera3dDepthLoadOp> for PyCamera3dDepthLoadOp {
    fn from(op: Camera3dDepthLoadOp) -> Self {
        match op {
            Camera3dDepthLoadOp::Clear(val) => PyCamera3dDepthLoadOp::Clear(val),
            Camera3dDepthLoadOp::Load => PyCamera3dDepthLoadOp::Load(),
        }
    }
}

impl From<PyCamera3dDepthLoadOp> for Camera3dDepthLoadOp {
    fn from(op: PyCamera3dDepthLoadOp) -> Self {
        match op {
            PyCamera3dDepthLoadOp::Clear(val) => Camera3dDepthLoadOp::Clear(val),
            PyCamera3dDepthLoadOp::Load() => Camera3dDepthLoadOp::Load,
        }
    }
}
