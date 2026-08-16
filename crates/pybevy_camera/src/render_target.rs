use bevy::{
    camera::{ImageRenderTarget, ManualTextureViewHandle, RenderTarget},
    math::UVec2,
    window::WindowRef,
};
use pybevy_core::{ComponentStorage, PyEntity, PyHandle};
use pybevy_macros::pyenum;
use pybevy_math::uvec2::PyUVec2;
use pybevy_window::window_ref::PyWindowRef;
use pyo3::prelude::*;

use crate::{
    image_render_target::PyImageRenderTarget,
    manual_texture_view_handle::PyManualTextureViewHandle,
    normalized_render_target::PyNormalizedRenderTarget,
};

#[pyenum(RenderTarget, component)]
#[pyclass(name = "RenderTarget", module = "pybevy.camera")]
pub enum PyRenderTarget {
    #[py_bevy(tuple)]
    Window {
        #[py_type(PyWindowRef)]
        value: WindowRef,
    },
    #[py_bevy(tuple)]
    Image {
        #[py_type(PyImageRenderTarget)]
        value: ImageRenderTarget,
    },
    #[py_bevy(tuple)]
    TextureView {
        #[py_type(PyManualTextureViewHandle)]
        value: ManualTextureViewHandle,
    },
    #[pyo3(name = "None_")]
    None {
        #[py_type(PyUVec2)]
        #[py_try_into]
        size: UVec2,
    },
}

#[pymethods]
impl PyRenderTarget {
    pub fn as_image(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.as_image().map(PyHandle::from))
    }

    #[pyo3(signature = (primary_window = None))]
    pub fn normalize(
        &self,
        primary_window: Option<&PyEntity>,
    ) -> PyResult<Option<PyNormalizedRenderTarget>> {
        Ok(self
            .as_ref()?
            .normalize(primary_window.map(|entity| entity.0))
            .map(Into::into))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()?.reborrow() {
            RenderTarget::Window(value) => Ok(format!("RenderTarget.Window({value:?})")),
            RenderTarget::Image(value) => Ok(format!("RenderTarget.Image({value:?})")),
            RenderTarget::TextureView(value) => Ok(format!("RenderTarget.TextureView({value:?})")),
            RenderTarget::None { size } => Ok(format!(
                "RenderTarget.None_(size=UVec2({}, {}))",
                size.x, size.y
            )),
        }
    }
}
