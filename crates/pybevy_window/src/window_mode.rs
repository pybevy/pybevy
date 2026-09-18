use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{monitor_selection::PyMonitorSelection, video_mode_selection::PyVideoModeSelection};

#[pyenum(WindowMode, empty_tuple, no_repr)]
#[pyclass(
    name = "WindowMode",
    module = "pybevy.window",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyWindowMode {
    Windowed(),
    #[py_bevy(tuple)]
    BorderlessFullscreen {
        #[py_type(PyMonitorSelection)]
        monitor: MonitorSelection,
    },
    #[py_bevy(tuple)]
    Fullscreen {
        #[py_type(PyMonitorSelection)]
        monitor: MonitorSelection,
        #[py_type(PyVideoModeSelection)]
        video_mode: VideoModeSelection,
    },
}

impl Default for PyWindowMode {
    fn default() -> Self {
        PyWindowMode::Windowed()
    }
}

#[pymethods]
impl PyWindowMode {
    pub fn __repr__(&self) -> String {
        match self {
            PyWindowMode::Windowed() => "WindowMode.Windowed()".to_string(),
            PyWindowMode::BorderlessFullscreen { monitor } => {
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
