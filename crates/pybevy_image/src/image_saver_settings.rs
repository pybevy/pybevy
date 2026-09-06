use bevy::image::{ImageSaverSettings, SaveImageFormatSetting};
use pyo3::prelude::*;

use crate::save_image_format_setting::PySaveImageFormatSetting;

#[pyclass(name = "ImageSaverSettings", module = "pybevy.image", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyImageSaverSettings {
    inner: ImageSaverSettings,
}

#[pymethods]
impl PyImageSaverSettings {
    #[new]
    #[pyo3(signature = (format = PySaveImageFormatSetting::FromExtension()))]
    pub fn new(format: PySaveImageFormatSetting) -> Self {
        Self {
            inner: ImageSaverSettings {
                format: format.into(),
            },
        }
    }

    #[getter]
    pub fn format(&self) -> PySaveImageFormatSetting {
        self.inner.format.into()
    }

    #[setter]
    pub fn set_format(&mut self, format: PySaveImageFormatSetting) {
        self.inner.format = format.into();
    }

    pub fn __repr__(&self) -> String {
        let format: SaveImageFormatSetting = self.inner.format;
        format!("ImageSaverSettings(format={format:?})")
    }
}

impl From<ImageSaverSettings> for PyImageSaverSettings {
    fn from(inner: ImageSaverSettings) -> Self {
        Self { inner }
    }
}

impl From<PyImageSaverSettings> for ImageSaverSettings {
    fn from(py: PyImageSaverSettings) -> Self {
        py.inner
    }
}
