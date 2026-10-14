use bevy::{
    camera::{Camera3d, Camera3dDepthLoadOp},
    render::render_resource::TextureUsages,
};
use pybevy_core::{
    ComponentStorage, PyComponent,
    domain::{DomainViolation, FieldDomain, check_for},
};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::{
    camera_3d_depth_load_op::PyCamera3dDepthLoadOp,
    camera_3d_depth_texture_usage::PyCamera3dDepthTextureUsage,
    texture_usage_rules::check_depth_texture_usage,
};

const DEFAULT_DEPTH_TEXTURE_USAGE: u32 = TextureUsages::RENDER_ATTACHMENT.bits();
const DEFAULT_DEPTH_CLEAR_VALUE: f32 = 0.0;
/// wgpu rejects a depth clear value outside `0.0..=1.0` when the pass begins.
const DEPTH_CLEAR_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: 0.0, hi: 1.0 }];

fn check_depth_load_op(op: Camera3dDepthLoadOp) -> Result<Camera3dDepthLoadOp, DomainViolation> {
    if let Camera3dDepthLoadOp::Clear(value) = op {
        check_for::<PyCamera3d, _>("depth_load_op", value, DEPTH_CLEAR_DOMAIN)?;
    }
    Ok(op)
}

fn validate_camera_3d(camera: &Camera3d) -> Result<(), String> {
    check_depth_load_op(camera.depth_load_op.clone()).map_err(|error| error.to_string())?;
    check_depth_texture_usage(camera.depth_texture_usages.0).map(drop)
}

#[pycomponent(Camera3d, bridge, reflect_validate = validate_camera_3d)]
#[pyclass(name = "Camera3d", module = "pybevy.camera", extends = PyComponent)]
pub struct PyCamera3d {
    pub(crate) storage: ComponentStorage<Camera3d>,
}

#[pymethods]
impl PyCamera3d {
    #[new]
    #[pyo3(signature = (
        *,
        depth_load_op = PyCamera3dDepthLoadOp::Clear { value: DEFAULT_DEPTH_CLEAR_VALUE },
        depth_texture_usages = PyCamera3dDepthTextureUsage(DEFAULT_DEPTH_TEXTURE_USAGE)
    ))]
    pub fn new(
        depth_load_op: PyCamera3dDepthLoadOp,
        depth_texture_usages: PyCamera3dDepthTextureUsage,
    ) -> PyResult<PyClassInitializer<Self>> {
        let camera = Camera3d {
            depth_load_op: check_depth_load_op(depth_load_op.into())?,
            depth_texture_usages: TextureUsages::from_bits_truncate(depth_texture_usages.0).into(),
        };

        Ok(Self::from_owned(camera).into())
    }

    #[getter]
    pub fn depth_load_op(&self) -> PyResult<PyCamera3dDepthLoadOp> {
        Ok(self.as_ref()?.depth_load_op.clone().into())
    }

    #[setter]
    pub fn set_depth_load_op(&mut self, value: PyCamera3dDepthLoadOp) -> PyResult<()> {
        self.as_mut()?.depth_load_op = check_depth_load_op(value.into())?;
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
