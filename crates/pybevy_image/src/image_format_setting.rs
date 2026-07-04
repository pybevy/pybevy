use bevy::image::{ImageFormat, ImageFormatSetting};
use pyo3::prelude::*;

use crate::image_format::PyImageFormat;
#[pyclass(name = "ImageFormatSetting", eq, frozen, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyImageFormatSetting {
    FromExtension(),
    Guess(),
}

impl From<ImageFormatSetting> for PyImageFormatSetting {
    fn from(setting: ImageFormatSetting) -> Self {
        match setting {
            ImageFormatSetting::FromExtension => PyImageFormatSetting::FromExtension(),
            ImageFormatSetting::Guess => PyImageFormatSetting::Guess(),
            ImageFormatSetting::Format(_) => {
                // Format variant loses the specific format info but we map to FromExtension
                PyImageFormatSetting::FromExtension()
            }
        }
    }
}

impl From<PyImageFormatSetting> for ImageFormatSetting {
    fn from(setting: PyImageFormatSetting) -> Self {
        match setting {
            PyImageFormatSetting::FromExtension() => ImageFormatSetting::FromExtension,
            PyImageFormatSetting::Guess() => ImageFormatSetting::Guess,
        }
    }
}

#[pymethods]
impl PyImageFormatSetting {
    #[staticmethod]
    pub fn from_extension() -> Self {
        PyImageFormatSetting::FromExtension()
    }
    #[staticmethod]
    pub fn guess() -> Self {
        PyImageFormatSetting::Guess()
    }
    #[staticmethod]
    pub fn format(format: PyImageFormat) -> PyImageFormatSettingWithFormat {
        PyImageFormatSettingWithFormat {
            format: format.into(),
        }
    }

    pub fn __repr__(&self) -> String {
        match self {
            PyImageFormatSetting::FromExtension() => "ImageFormatSetting.FromExtension".to_string(),
            PyImageFormatSetting::Guess() => "ImageFormatSetting.Guess".to_string(),
        }
    }
}

/// Image format setting with a specific format.
#[pyclass(name = "ImageFormatSettingWithFormat", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyImageFormatSettingWithFormat {
    pub(crate) format: ImageFormat,
}

impl From<PyImageFormatSettingWithFormat> for ImageFormatSetting {
    fn from(setting: PyImageFormatSettingWithFormat) -> Self {
        ImageFormatSetting::Format(setting.format)
    }
}
