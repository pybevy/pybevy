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
    #[pyo3(constructor = (*, width, height))]
    Fixed {
        width: f32,
        height: f32,
    },
    #[pyo3(constructor = (*, min_width, min_height))]
    AutoMin {
        min_width: f32,
        min_height: f32,
    },
    #[pyo3(constructor = (*, max_width, max_height))]
    AutoMax {
        max_width: f32,
        max_height: f32,
    },
    #[pyo3(constructor = (*, viewport_height))]
    FixedVertical {
        viewport_height: f32,
    },
    #[pyo3(constructor = (*, viewport_width))]
    FixedHorizontal {
        viewport_width: f32,
    },
}

impl Default for PyScalingMode {
    fn default() -> Self {
        Self::WindowSize()
    }
}
