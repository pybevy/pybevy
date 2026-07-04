use std::path::PathBuf;

use bevy::window::FileDragAndDrop;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum FileDragAndDropData {
    DroppedFile { window: PyEntity, path: PathBuf },
    HoveredFile { window: PyEntity, path: PathBuf },
    HoveredFileCanceled { window: PyEntity },
}

#[pymessage(FileDragAndDrop)]
#[pyclass(name = "FileDragAndDrop", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyFileDragAndDrop {
    data: FileDragAndDropData,
}

impl PyFileDragAndDrop {
    pub fn from_bevy(event: &FileDragAndDrop) -> (Self, PyMessage) {
        (Self::from(event), PyMessage)
    }
}

impl From<&FileDragAndDrop> for PyFileDragAndDrop {
    fn from(event: &FileDragAndDrop) -> Self {
        let data = match event {
            FileDragAndDrop::DroppedFile { window, path_buf } => FileDragAndDropData::DroppedFile {
                window: (*window).into(),
                path: path_buf.clone(),
            },
            FileDragAndDrop::HoveredFile { window, path_buf } => FileDragAndDropData::HoveredFile {
                window: (*window).into(),
                path: path_buf.clone(),
            },
            FileDragAndDrop::HoveredFileCanceled { window } => {
                FileDragAndDropData::HoveredFileCanceled {
                    window: (*window).into(),
                }
            }
        };
        PyFileDragAndDrop { data }
    }
}

#[pymethods]
impl PyFileDragAndDrop {
    pub fn is_dropped_file(&self) -> bool {
        matches!(self.data, FileDragAndDropData::DroppedFile { .. })
    }

    pub fn is_hovered_file(&self) -> bool {
        matches!(self.data, FileDragAndDropData::HoveredFile { .. })
    }

    pub fn is_hovered_file_canceled(&self) -> bool {
        matches!(self.data, FileDragAndDropData::HoveredFileCanceled { .. })
    }

    #[getter]
    pub fn window(&self) -> PyEntity {
        match &self.data {
            FileDragAndDropData::DroppedFile { window, .. } => *window,
            FileDragAndDropData::HoveredFile { window, .. } => *window,
            FileDragAndDropData::HoveredFileCanceled { window } => *window,
        }
    }

    #[getter]
    pub fn path(&self) -> Option<String> {
        match &self.data {
            FileDragAndDropData::DroppedFile { path, .. } => {
                Some(path.to_string_lossy().to_string())
            }
            FileDragAndDropData::HoveredFile { path, .. } => {
                Some(path.to_string_lossy().to_string())
            }
            FileDragAndDropData::HoveredFileCanceled { .. } => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match &self.data {
            FileDragAndDropData::DroppedFile { window, path } => {
                format!(
                    "FileDragAndDrop.DroppedFile(window={window:?}, path={:?})",
                    path.display()
                )
            }
            FileDragAndDropData::HoveredFile { window, path } => {
                format!(
                    "FileDragAndDrop.HoveredFile(window={window:?}, path={:?})",
                    path.display()
                )
            }
            FileDragAndDropData::HoveredFileCanceled { window } => {
                format!("FileDragAndDrop.HoveredFileCanceled(window={window:?})")
            }
        }
    }
}
