use bevy::image::ImageFormatSetting;
use pyo3::prelude::*;

use crate::image_format::PyImageFormat;
#[pyclass(
    name = "ImageFormatSetting",
    module = "pybevy.image",
    eq,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyImageFormatSetting {
    FromExtension(),
    Guess(),
    Format { value: PyImageFormat },
}

impl From<ImageFormatSetting> for PyImageFormatSetting {
    fn from(setting: ImageFormatSetting) -> Self {
        match setting {
            ImageFormatSetting::FromExtension => PyImageFormatSetting::FromExtension(),
            ImageFormatSetting::Guess => PyImageFormatSetting::Guess(),
            ImageFormatSetting::Format(value) => PyImageFormatSetting::Format {
                value: value.into(),
            },
        }
    }
}

impl From<PyImageFormatSetting> for ImageFormatSetting {
    fn from(setting: PyImageFormatSetting) -> Self {
        match setting {
            PyImageFormatSetting::FromExtension() => ImageFormatSetting::FromExtension,
            PyImageFormatSetting::Guess() => ImageFormatSetting::Guess,
            PyImageFormatSetting::Format { value } => ImageFormatSetting::Format(value.into()),
        }
    }
}

#[pymethods]
impl PyImageFormatSetting {
    pub fn __repr__(&self) -> String {
        match self {
            PyImageFormatSetting::FromExtension() => "ImageFormatSetting.FromExtension".to_string(),
            PyImageFormatSetting::Guess() => "ImageFormatSetting.Guess".to_string(),
            PyImageFormatSetting::Format { value } => {
                format!("ImageFormatSetting.Format({value:?})")
            }
        }
    }
}
