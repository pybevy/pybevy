use bevy::camera::NormalizedRenderTarget;
use pybevy_macros::pyenum;
use pybevy_window::window_ref::PyNormalizedWindowRef;
use pyo3::prelude::*;

use crate::{
    image_render_target::PyImageRenderTarget, manual_texture_view_handle::PyManualTextureViewHandle,
};

#[pyenum(NormalizedRenderTarget, manual)]
#[pyclass(
    name = "NormalizedRenderTarget",
    module = "pybevy.camera",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PyNormalizedRenderTarget {
    Window {
        value: PyNormalizedWindowRef,
    },
    Image {
        value: PyImageRenderTarget,
    },
    TextureView {
        value: PyManualTextureViewHandle,
    },
    #[pyo3(name = "None_")]
    None {
        width: u32,
        height: u32,
    },
}

impl From<NormalizedRenderTarget> for PyNormalizedRenderTarget {
    fn from(value: NormalizedRenderTarget) -> Self {
        match value {
            NormalizedRenderTarget::Window(value) => Self::Window {
                value: value.into(),
            },
            NormalizedRenderTarget::Image(value) => Self::Image {
                value: value.into(),
            },
            NormalizedRenderTarget::TextureView(value) => Self::TextureView {
                value: value.into(),
            },
            NormalizedRenderTarget::None { width, height } => Self::None { width, height },
        }
    }
}

impl From<PyNormalizedRenderTarget> for NormalizedRenderTarget {
    fn from(value: PyNormalizedRenderTarget) -> Self {
        match value {
            PyNormalizedRenderTarget::Window { value } => Self::Window(value.into()),
            PyNormalizedRenderTarget::Image { value } => Self::Image(value.into()),
            PyNormalizedRenderTarget::TextureView { value } => Self::TextureView(value.into()),
            PyNormalizedRenderTarget::None { width, height } => Self::None { width, height },
        }
    }
}

#[pymethods]
impl PyNormalizedRenderTarget {
    fn __repr__(&self) -> String {
        match self {
            Self::Window { value } => format!("NormalizedRenderTarget.Window({value:?})"),
            Self::Image { value } => format!("NormalizedRenderTarget.Image({value:?})"),
            Self::TextureView { value } => {
                format!("NormalizedRenderTarget.TextureView({value:?})")
            }
            Self::None { width, height } => {
                format!("NormalizedRenderTarget.None_(width={width}, height={height})")
            }
        }
    }
}
