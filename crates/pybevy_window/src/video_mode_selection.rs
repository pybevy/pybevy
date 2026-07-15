use bevy::window::VideoModeSelection;
use pyo3::prelude::*;

use super::video_mode::PyVideoMode;

#[pyclass(name = "VideoModeSelection", eq, frozen, from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyVideoModeSelection {
    Current(),
    Specific { video_mode: PyVideoMode },
}

impl Default for PyVideoModeSelection {
    fn default() -> Self {
        PyVideoModeSelection::Current()
    }
}

impl From<PyVideoModeSelection> for VideoModeSelection {
    fn from(value: PyVideoModeSelection) -> Self {
        match value {
            PyVideoModeSelection::Current() => VideoModeSelection::Current,
            PyVideoModeSelection::Specific { video_mode } => {
                VideoModeSelection::Specific(video_mode.into())
            }
        }
    }
}

impl From<VideoModeSelection> for PyVideoModeSelection {
    fn from(value: VideoModeSelection) -> Self {
        match value {
            VideoModeSelection::Current => PyVideoModeSelection::Current(),
            VideoModeSelection::Specific(video_mode) => PyVideoModeSelection::Specific {
                video_mode: video_mode.into(),
            },
        }
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
