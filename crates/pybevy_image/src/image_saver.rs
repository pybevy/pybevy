use bevy::image::ImageSaver;
use pyo3::prelude::*;

/// Bevy's `AssetSaver` for `Image`, used with `pybevy.assets.save_using_saver`.
#[pyclass(name = "ImageSaver", module = "pybevy.image", frozen, from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyImageSaver;

#[pymethods]
impl PyImageSaver {
    #[new]
    pub fn new() -> Self {
        Self
    }

    fn __copy__(&self) -> Self {
        *self
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        *self
    }

    pub fn __repr__(&self) -> &'static str {
        "ImageSaver()"
    }
}

impl Default for PyImageSaver {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&PyImageSaver> for ImageSaver {
    fn from(_: &PyImageSaver) -> Self {
        ImageSaver
    }
}
