use bevy::window::WindowMode;
use pyo3::prelude::*;

use crate::{monitor_selection::PyMonitorSelection, video_mode_selection::PyVideoModeSelection};

#[pyclass(name = "WindowMode", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyWindowMode {
    Windowed(),
    BorderlessFullscreen(PyMonitorSelection),
    Fullscreen {
        monitor: PyMonitorSelection,
        video_mode: PyVideoModeSelection,
    },
}

impl Default for PyWindowMode {
    fn default() -> Self {
        PyWindowMode::Windowed()
    }
}

impl From<WindowMode> for PyWindowMode {
    fn from(mode: WindowMode) -> Self {
        match mode {
            WindowMode::Windowed => PyWindowMode::Windowed(),
            WindowMode::BorderlessFullscreen(monitor) => {
                PyWindowMode::BorderlessFullscreen(monitor.into())
            }
            WindowMode::Fullscreen(monitor, video_mode) => PyWindowMode::Fullscreen {
                monitor: monitor.into(),
                video_mode: video_mode.into(),
            },
        }
    }
}

impl From<PyWindowMode> for WindowMode {
    fn from(mode: PyWindowMode) -> Self {
        match mode {
            PyWindowMode::Windowed() => WindowMode::Windowed,
            PyWindowMode::BorderlessFullscreen(monitor) => {
                WindowMode::BorderlessFullscreen(monitor.into())
            }
            PyWindowMode::Fullscreen {
                monitor,
                video_mode,
            } => WindowMode::Fullscreen(monitor.into(), video_mode.into()),
        }
    }
}

#[pymethods]
impl PyWindowMode {
    fn __repr__(&self) -> String {
        match self {
            PyWindowMode::Windowed() => "WindowMode.Windowed()".to_string(),
            PyWindowMode::BorderlessFullscreen(monitor) => {
                format!("WindowMode.BorderlessFullscreen({:?})", monitor)
            }
            PyWindowMode::Fullscreen {
                monitor,
                video_mode,
            } => {
                format!(
                    "WindowMode.Fullscreen(monitor={:?}, video_mode={:?})",
                    monitor, video_mode
                )
            }
        }
    }
}
