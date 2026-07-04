use bevy::{camera::Camera3d, render::render_resource::TextureUsages};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::{
    camera_3d_depth_load_op::PyCamera3dDepthLoadOp,
    camera_3d_depth_texture_usage::PyCamera3dDepthTextureUsage,
};

const DEFAULT_DEPTH_TEXTURE_USAGE: u32 = TextureUsages::RENDER_ATTACHMENT.bits();
const DEFAULT_DEPTH_CLEAR_VALUE: f32 = 0.0;

#[pycomponent(Camera3d, bridge)]
#[pyclass(name = "Camera3d", extends = PyComponent, eq)]
pub struct PyCamera3d {
    pub(crate) storage: ComponentStorage<Camera3d>,
}

impl PartialEq for PyCamera3d {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => {
                PyCamera3dDepthLoadOp::from(a.depth_load_op.clone())
                    == PyCamera3dDepthLoadOp::from(b.depth_load_op.clone())
                    && a.depth_texture_usages.0 == b.depth_texture_usages.0
            }
            _ => false,
        }
    }
}

#[pymethods]
impl PyCamera3d {
    #[new]
    #[pyo3(signature = (
        depth_load_op = PyCamera3dDepthLoadOp::Clear(DEFAULT_DEPTH_CLEAR_VALUE),
        depth_texture_usages = PyCamera3dDepthTextureUsage(DEFAULT_DEPTH_TEXTURE_USAGE)
    ))]
    pub fn new(
        depth_load_op: PyCamera3dDepthLoadOp,
        depth_texture_usages: PyCamera3dDepthTextureUsage,
    ) -> PyClassInitializer<Self> {
        let camera = Camera3d {
            depth_load_op: depth_load_op.into(),
            depth_texture_usages: TextureUsages::from_bits_truncate(depth_texture_usages.0).into(),
        };

        Self::from_owned(camera).into()
    }

    #[getter]
    pub fn depth_load_op(&self) -> PyResult<PyCamera3dDepthLoadOp> {
        Ok(self.as_ref()?.depth_load_op.clone().into())
    }

    #[setter]
    pub fn set_depth_load_op(&mut self, value: PyCamera3dDepthLoadOp) -> PyResult<()> {
        self.as_mut()?.depth_load_op = value.into();
        Ok(())
    }

    #[getter]
    pub fn depth_texture_usages(&self) -> PyResult<PyCamera3dDepthTextureUsage> {
        Ok(PyCamera3dDepthTextureUsage(
            self.as_ref()?.depth_texture_usages.0,
        ))
    }

    #[setter]
    pub fn set_depth_texture_usages(&mut self, value: PyCamera3dDepthTextureUsage) -> PyResult<()> {
        self.as_mut()?.depth_texture_usages = TextureUsages::from_bits_truncate(value.0).into();
        Ok(())
    }
}
