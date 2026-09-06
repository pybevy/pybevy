use bevy::camera::ScalingMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ScalingMode, empty_tuple, unit_parens)]
#[pyclass(
    name = "ScalingMode",
    module = "pybevy.camera",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyScalingMode {
    WindowSize(),
    Fixed { width: f32, height: f32 },
    AutoMin { min_width: f32, min_height: f32 },
    AutoMax { max_width: f32, max_height: f32 },
    FixedVertical { viewport_height: f32 },
    FixedHorizontal { viewport_width: f32 },
}

impl Default for PyScalingMode {
    fn default() -> Self {
        Self::WindowSize()
    }
}
