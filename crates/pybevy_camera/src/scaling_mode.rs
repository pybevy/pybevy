use bevy::camera::ScalingMode;
use pyo3::prelude::*;

#[pyclass(name = "ScalingMode")]
#[derive(Debug, Clone)]
pub struct PyScalingMode {
    pub(crate) inner: ScalingMode,
}

impl From<ScalingMode> for PyScalingMode {
    fn from(mode: ScalingMode) -> Self {
        Self { inner: mode }
    }
}

impl From<PyScalingMode> for ScalingMode {
    fn from(mode: PyScalingMode) -> Self {
        mode.inner
    }
}

#[pymethods]
impl PyScalingMode {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: ScalingMode::WindowSize,
        }
    }

    #[staticmethod]
    #[pyo3(name = "WindowSize")]
    pub fn window_size() -> Self {
        Self {
            inner: ScalingMode::WindowSize,
        }
    }

    #[staticmethod]
    #[pyo3(name = "Fixed")]
    pub fn fixed(width: f32, height: f32) -> Self {
        Self {
            inner: ScalingMode::Fixed { width, height },
        }
    }

    #[staticmethod]
    #[pyo3(name = "AutoMin")]
    pub fn auto_min(min_width: f32, min_height: f32) -> Self {
        Self {
            inner: ScalingMode::AutoMin {
                min_width,
                min_height,
            },
        }
    }

    #[staticmethod]
    #[pyo3(name = "AutoMax")]
    pub fn auto_max(max_width: f32, max_height: f32) -> Self {
        Self {
            inner: ScalingMode::AutoMax {
                max_width,
                max_height,
            },
        }
    }

    #[staticmethod]
    #[pyo3(name = "FixedVertical")]
    pub fn fixed_vertical(viewport_height: f32) -> Self {
        Self {
            inner: ScalingMode::FixedVertical { viewport_height },
        }
    }

    #[staticmethod]
    #[pyo3(name = "FixedHorizontal")]
    pub fn fixed_horizontal(viewport_width: f32) -> Self {
        Self {
            inner: ScalingMode::FixedHorizontal { viewport_width },
        }
    }
}

impl Default for PyScalingMode {
    fn default() -> Self {
        Self::new()
    }
}
