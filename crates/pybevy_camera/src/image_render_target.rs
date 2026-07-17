use std::hash::{Hash, Hasher};

use bevy::camera::ImageRenderTarget;
use pybevy_core::{PyHandle, extract_handle_from_any};
use pyo3::prelude::*;

#[pyclass(
    name = "ImageRenderTarget",
    module = "pybevy.camera",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyImageRenderTarget {
    pub(crate) inner: ImageRenderTarget,
}

impl Hash for PyImageRenderTarget {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl From<ImageRenderTarget> for PyImageRenderTarget {
    fn from(value: ImageRenderTarget) -> Self {
        Self { inner: value }
    }
}

impl From<PyImageRenderTarget> for ImageRenderTarget {
    fn from(value: PyImageRenderTarget) -> Self {
        value.inner
    }
}

#[pymethods]
impl PyImageRenderTarget {
    #[new]
    #[pyo3(signature = (handle, scale_factor = 1.0))]
    pub fn new(handle: &Bound<'_, PyAny>, scale_factor: f32) -> PyResult<Self> {
        let handle = extract_handle_from_any(handle)?;
        Ok(Self {
            inner: ImageRenderTarget {
                handle: (&handle).try_into()?,
                scale_factor,
            },
        })
    }

    #[getter]
    pub fn handle(&self) -> PyHandle {
        PyHandle::from(&self.inner.handle)
    }

    #[getter]
    pub fn scale_factor(&self) -> f32 {
        self.inner.scale_factor
    }

    fn __repr__(&self) -> String {
        format!(
            "ImageRenderTarget(handle={:?}, scale_factor={})",
            self.inner.handle, self.inner.scale_factor
        )
    }
}
