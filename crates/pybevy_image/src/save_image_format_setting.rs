use bevy::image::{ImageFormat, SaveImageFormatSetting};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::image_format::PyImageFormat;

#[pyenum(SaveImageFormatSetting, empty_tuple, no_repr)]
#[pyclass(
    name = "SaveImageFormatSetting",
    module = "pybevy.image",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PySaveImageFormatSetting {
    FromExtension(),
    #[py_bevy(tuple)]
    Format {
        #[py_type(PyImageFormat)]
        value: ImageFormat,
    },
}

#[pymethods]
impl PySaveImageFormatSetting {
    fn __repr__(&self) -> String {
        match self {
            PySaveImageFormatSetting::FromExtension() => {
                "SaveImageFormatSetting.FromExtension".to_string()
            }
            PySaveImageFormatSetting::Format { value } => {
                format!("SaveImageFormatSetting.Format({value:?})")
            }
        }
    }
}
