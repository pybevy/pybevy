use bevy::{ecs::entity::Entity, window::FileDragAndDrop};
use pybevy_core::PyEntity;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum FileDragAndDropValue {
    DroppedFile { window: Entity, path_buf: String },
    HoveredFile { window: Entity, path_buf: String },
    HoveredFileCanceled { window: Entity },
}

impl From<&FileDragAndDrop> for FileDragAndDropValue {
    fn from(value: &FileDragAndDrop) -> Self {
        match value {
            FileDragAndDrop::DroppedFile { window, path_buf } => Self::DroppedFile {
                window: *window,
                path_buf: path_buf.to_string_lossy().into_owned(),
            },
            FileDragAndDrop::HoveredFile { window, path_buf } => Self::HoveredFile {
                window: *window,
                path_buf: path_buf.to_string_lossy().into_owned(),
            },
            FileDragAndDrop::HoveredFileCanceled { window } => {
                Self::HoveredFileCanceled { window: *window }
            }
        }
    }
}

/// The mirror stores paths as `String`, so returning to bevy re-wraps them.
/// A path that was not valid UTF-8 already lost that on the way out.
impl From<FileDragAndDropValue> for FileDragAndDrop {
    fn from(value: FileDragAndDropValue) -> Self {
        match value {
            FileDragAndDropValue::DroppedFile { window, path_buf } => {
                FileDragAndDrop::DroppedFile {
                    window,
                    path_buf: path_buf.into(),
                }
            }
            FileDragAndDropValue::HoveredFile { window, path_buf } => {
                FileDragAndDrop::HoveredFile {
                    window,
                    path_buf: path_buf.into(),
                }
            }
            FileDragAndDropValue::HoveredFileCanceled { window } => {
                FileDragAndDrop::HoveredFileCanceled { window }
            }
        }
    }
}

#[pyenum(FileDragAndDrop, message, mirror = FileDragAndDropValue)]
#[pyclass(module = "pybevy.window", name = "FileDragAndDrop")]
pub enum PyFileDragAndDrop {
    DroppedFile {
        #[py_type(PyEntity)]
        window: Entity,
        path_buf: String,
    },
    HoveredFile {
        #[py_type(PyEntity)]
        window: Entity,
        path_buf: String,
    },
    HoveredFileCanceled {
        #[py_type(PyEntity)]
        window: Entity,
    },
}
