use bevy::{
    camera::{ImageRenderTarget, ManualTextureViewHandle, NormalizedRenderTarget},
    window::NormalizedWindowRef,
};
use pybevy_macros::pyenum;
use pybevy_window::window_ref::PyNormalizedWindowRef;
use pyo3::prelude::*;

use crate::{
    image_render_target::PyImageRenderTarget, manual_texture_view_handle::PyManualTextureViewHandle,
};

#[pyenum(NormalizedRenderTarget, no_repr)]
#[pyclass(
    name = "NormalizedRenderTarget",
    module = "pybevy.camera",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyNormalizedRenderTarget {
    #[py_bevy(tuple)]
    Window {
        #[py_type(PyNormalizedWindowRef)]
        value: NormalizedWindowRef,
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
    None { width: u32, height: u32 },
}

#[pymethods]
impl PyNormalizedRenderTarget {
    fn __repr__(&self) -> String {
        match self {
            Self::Window { value } => {
                let value: PyNormalizedWindowRef = (*value).into();
                format!("NormalizedRenderTarget.Window({value:?})")
            }
            Self::Image { value } => {
                let value: PyImageRenderTarget = value.clone().into();
                format!("NormalizedRenderTarget.Image({value:?})")
            }
            Self::TextureView { value } => {
                let value: PyManualTextureViewHandle = (*value).into();
                format!("NormalizedRenderTarget.TextureView({value:?})")
            }
            Self::None { width, height } => {
                format!("NormalizedRenderTarget.None_(width={width}, height={height})")
            }
        }
    }
}
