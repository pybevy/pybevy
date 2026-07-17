use bevy::camera::ScalingMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ScalingMode, manual)]
#[pyclass(name = "ScalingMode", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyScalingMode {
    WindowSize(),
    Fixed { width: f32, height: f32 },
    AutoMin { min_width: f32, min_height: f32 },
    AutoMax { max_width: f32, max_height: f32 },
    FixedVertical { viewport_height: f32 },
    FixedHorizontal { viewport_width: f32 },
}

impl From<ScalingMode> for PyScalingMode {
    fn from(mode: ScalingMode) -> Self {
        match mode {
            ScalingMode::WindowSize => Self::WindowSize(),
            ScalingMode::Fixed { width, height } => Self::Fixed { width, height },
            ScalingMode::AutoMin {
                min_width,
                min_height,
            } => Self::AutoMin {
                min_width,
                min_height,
            },
            ScalingMode::AutoMax {
                max_width,
                max_height,
            } => Self::AutoMax {
                max_width,
                max_height,
            },
            ScalingMode::FixedVertical { viewport_height } => {
                Self::FixedVertical { viewport_height }
            }
            ScalingMode::FixedHorizontal { viewport_width } => {
                Self::FixedHorizontal { viewport_width }
            }
        }
    }
}

impl From<PyScalingMode> for ScalingMode {
    fn from(mode: PyScalingMode) -> Self {
        match mode {
            PyScalingMode::WindowSize() => Self::WindowSize,
            PyScalingMode::Fixed { width, height } => Self::Fixed { width, height },
            PyScalingMode::AutoMin {
                min_width,
                min_height,
            } => Self::AutoMin {
                min_width,
                min_height,
            },
            PyScalingMode::AutoMax {
                max_width,
                max_height,
            } => Self::AutoMax {
                max_width,
                max_height,
            },
            PyScalingMode::FixedVertical { viewport_height } => {
                Self::FixedVertical { viewport_height }
            }
            PyScalingMode::FixedHorizontal { viewport_width } => {
                Self::FixedHorizontal { viewport_width }
            }
        }
    }
}

#[pymethods]
impl PyScalingMode {
    fn __repr__(&self) -> String {
        match self {
            Self::WindowSize() => "ScalingMode.WindowSize()".to_string(),
            Self::Fixed { width, height } => {
                format!("ScalingMode.Fixed(width={width}, height={height})")
            }
            Self::AutoMin {
                min_width,
                min_height,
            } => {
                format!("ScalingMode.AutoMin(min_width={min_width}, min_height={min_height})")
            }
            Self::AutoMax {
                max_width,
                max_height,
            } => {
                format!("ScalingMode.AutoMax(max_width={max_width}, max_height={max_height})")
            }
            Self::FixedVertical { viewport_height } => {
                format!("ScalingMode.FixedVertical(viewport_height={viewport_height})")
            }
            Self::FixedHorizontal { viewport_width } => {
                format!("ScalingMode.FixedHorizontal(viewport_width={viewport_width})")
            }
        }
    }
}

impl Default for PyScalingMode {
    fn default() -> Self {
        Self::WindowSize()
    }
}
