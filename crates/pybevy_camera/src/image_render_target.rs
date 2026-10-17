use bevy::camera::ImageRenderTarget;
use pybevy_core::{
    PyHandle,
    domain::{FieldDomain, check_for},
    extract_handle_from_any,
    repr::{float_repr, object_repr},
};
use pyo3::prelude::*;

// UiScale's range: tiny factors make the logical size infinite, huge ones overflow glyph rasterization.
const SCALE_FACTOR_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive {
    lo: 1.0 / 64.0,
    hi: 64.0,
}];

#[pyclass(
    name = "ImageRenderTarget",
    module = "pybevy.camera",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PyImageRenderTarget {
    pub(crate) inner: ImageRenderTarget,
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
    #[pyo3(signature = (*, handle, scale_factor = 1.0))]
    pub fn new(handle: &Bound<'_, PyAny>, scale_factor: f32) -> PyResult<Self> {
        let scale_factor = check_for::<Self, _>("scale_factor", scale_factor, SCALE_FACTOR_DOMAIN)?;
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
            "ImageRenderTarget(handle={}, scale_factor={})",
            object_repr(PyHandle::from(&self.inner.handle)),
            float_repr(self.inner.scale_factor)
        )
    }
}
