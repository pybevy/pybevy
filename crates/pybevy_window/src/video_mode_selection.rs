use bevy::window::{VideoMode, VideoModeSelection};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use super::video_mode::PyVideoMode;

#[pyenum(VideoModeSelection, empty_tuple, no_repr)]
#[pyclass(
    name = "VideoModeSelection",
    module = "pybevy.window",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyVideoModeSelection {
    Current(),
    #[py_bevy(tuple)]
    Specific {
        #[py_type(PyVideoMode)]
        video_mode: VideoMode,
    },
}

impl Default for PyVideoModeSelection {
    fn default() -> Self {
        PyVideoModeSelection::Current()
    }
}

#[pymethods]
impl PyVideoModeSelection {
    fn __repr__(&self) -> String {
        match self {
            PyVideoModeSelection::Current() => "VideoModeSelection.Current()".to_string(),
            PyVideoModeSelection::Specific { video_mode } => {
                format!("VideoModeSelection.Specific({})", video_mode.__repr__())
            }
        }
    }
}
