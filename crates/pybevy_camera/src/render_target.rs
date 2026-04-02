use bevy::{
    asset::Handle,
    camera::{ImageRenderTarget, ManualTextureViewHandle, RenderTarget},
    image::Image,
    math::UVec2,
    window::WindowRef,
};
use pybevy_core::{ComponentStorage, PyComponent, PyEntity, PyHandle, extract_handle_from_any};
use pybevy_macros::component_storage;
use pybevy_math::uvec2::PyUVec2;
use pyo3::prelude::*;

use super::normalized_render_target::PyNormalizedRenderTarget;

#[component_storage(RenderTarget, bridge)]
#[pyclass(name = "RenderTarget", extends = PyComponent)]
#[derive(Clone)]
pub struct PyRenderTarget {
    pub(crate) storage: ComponentStorage<RenderTarget>,
}

#[pymethods]
impl PyRenderTarget {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(RenderTarget::Window(WindowRef::default()))
    }

    #[staticmethod]
    pub fn window(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(RenderTarget::Window(WindowRef::default())),
        )
    }

    #[staticmethod]
    pub fn image(py: Python<'_>, handle: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        let py_handle = extract_handle_from_any(handle)?;
        let bevy_handle: Handle<Image> = (&py_handle).try_into()?;
        Py::new(
            py,
            Self::from_owned(RenderTarget::Image(ImageRenderTarget {
                handle: bevy_handle,
                scale_factor: 1.0,
            })),
        )
    }

    #[staticmethod]
    pub fn texture_view(py: Python<'_>, id: u32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(RenderTarget::TextureView(ManualTextureViewHandle(id))),
        )
    }

    #[staticmethod]
    #[pyo3(name = "none")]
    pub fn none_(py: Python<'_>, size: PyUVec2) -> PyResult<Py<Self>> {
        let s: UVec2 = size.into();
        Py::new(py, Self::from_owned(RenderTarget::None { size: s }))
    }

    pub fn as_image(&self, py: Python<'_>) -> PyResult<Option<Py<PyHandle>>> {
        match self.as_ref()? {
            RenderTarget::Image(image_target) => {
                let py_handle = PyHandle::from(&image_target.handle);
                Ok(Some(Py::new(py, py_handle)?))
            }
            _ => Ok(None),
        }
    }

    pub fn normalize(
        &self,
        primary_window: Option<&PyEntity>,
    ) -> PyResult<Option<PyNormalizedRenderTarget>> {
        let target = self.as_ref()?;
        let primary = primary_window.map(|e| e.0);
        let normalized = target.normalize(primary);
        Ok(normalized.map(PyNormalizedRenderTarget::from))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()? {
            RenderTarget::Window(_) => Ok("RenderTarget.Window".to_string()),
            RenderTarget::Image(_) => Ok("RenderTarget.Image(...)".to_string()),
            RenderTarget::TextureView(handle) => {
                Ok(format!("RenderTarget.TextureView({})", handle.0))
            }
            RenderTarget::None { size } => Ok(format!("RenderTarget.None({}x{})", size.x, size.y)),
        }
    }
}
