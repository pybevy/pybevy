use std::hash::{Hash, Hasher};

use bevy::{
    asset::Handle,
    camera::{ImageRenderTarget, ManualTextureViewHandle, NormalizedRenderTarget},
    ecs::entity::ContainsEntity,
    image::Image,
};
use pybevy_core::{PyEntity, PyHandle};
use pyo3::prelude::*;

#[pyclass(from_py_object, name = "NormalizedRenderTarget", frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyNormalizedRenderTarget(pub(crate) NormalizedRenderTarget);

#[pymethods]
impl PyNormalizedRenderTarget {
    #[staticmethod]
    #[pyo3(signature = (handle, scale_factor = 1.0))]
    pub fn image(handle: &PyHandle, scale_factor: f32) -> PyResult<Self> {
        let image_handle = Handle::<Image>::try_from(handle)?;
        Ok(PyNormalizedRenderTarget(NormalizedRenderTarget::Image(
            ImageRenderTarget {
                handle: image_handle,
                scale_factor,
            },
        )))
    }

    #[staticmethod]
    pub fn texture_view(id: u32) -> Self {
        PyNormalizedRenderTarget(NormalizedRenderTarget::TextureView(
            ManualTextureViewHandle(id),
        ))
    }

    #[staticmethod]
    pub fn none(width: u32, height: u32) -> Self {
        PyNormalizedRenderTarget(NormalizedRenderTarget::None { width, height })
    }

    pub fn is_window(&self) -> bool {
        matches!(self.0, NormalizedRenderTarget::Window(_))
    }

    pub fn is_image(&self) -> bool {
        matches!(self.0, NormalizedRenderTarget::Image(_))
    }

    pub fn is_texture_view(&self) -> bool {
        matches!(self.0, NormalizedRenderTarget::TextureView(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self.0, NormalizedRenderTarget::None { .. })
    }

    pub fn window_entity(&self) -> Option<PyEntity> {
        match &self.0 {
            NormalizedRenderTarget::Window(window_ref) => Some(window_ref.entity().into()),
            _ => None,
        }
    }

    pub fn none_dimensions(&self) -> Option<(u32, u32)> {
        match &self.0 {
            NormalizedRenderTarget::None { width, height } => Some((*width, *height)),
            _ => None,
        }
    }

    pub fn texture_view_id(&self) -> Option<u32> {
        match &self.0 {
            NormalizedRenderTarget::TextureView(handle) => Some(handle.0),
            _ => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            NormalizedRenderTarget::Window(window_ref) => {
                use bevy::ecs::entity::ContainsEntity;
                format!("NormalizedRenderTarget.window({:?})", window_ref.entity())
            }
            NormalizedRenderTarget::Image(_) => "NormalizedRenderTarget.image(...)".to_string(),
            NormalizedRenderTarget::TextureView(handle) => {
                format!("NormalizedRenderTarget.texture_view({})", handle.0)
            }
            NormalizedRenderTarget::None { width, height } => {
                format!("NormalizedRenderTarget.none({}, {})", width, height)
            }
        }
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<NormalizedRenderTarget> for PyNormalizedRenderTarget {
    fn from(value: NormalizedRenderTarget) -> Self {
        PyNormalizedRenderTarget(value)
    }
}

impl From<PyNormalizedRenderTarget> for NormalizedRenderTarget {
    fn from(value: PyNormalizedRenderTarget) -> Self {
        value.0
    }
}
