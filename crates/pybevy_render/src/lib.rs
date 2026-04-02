pub mod alpha_mode;
pub mod atmosphere_mode;
pub mod color_grading;
pub mod color_grading_component;
pub mod face;
pub mod mip_bias;
pub mod msaa;
pub mod opaque_render_method;
pub mod plugin;
pub mod power_preference;
pub mod temporal_jitter;
pub mod unit_markers;

pub use alpha_mode::PyAlphaMode;
pub use atmosphere_mode::PyAtmosphereMode;
pub use color_grading::{PyColorGradingGlobal, PyColorGradingSection};
pub use color_grading_component::PyColorGrading;
pub use face::PyFace;
pub use mip_bias::PyMipBias;
pub use msaa::PyMsaa;
pub use opaque_render_method::PyOpaqueRenderMethod;
pub use plugin::PyRenderPlugin;
pub use power_preference::PyPowerPreference;
pub use pybevy_shader::{
    PyShader, PyShaderDefVal, PyShaderImport, PyShaderRef, PySource, PyValidateShader,
};
pub use pybevy_wgpu::{PyExtent3d, PyTextureDimension, PyTextureFormat};
use pyo3::prelude::*;
pub use temporal_jitter::PyTemporalJitter;
pub use unit_markers::{PyHdr, PyNoAutomaticBatching, PyNoIndirectDrawing, PyOcclusionCulling};

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "render")?;
    m.add_class::<PyRenderPlugin>()?;
    m.add_class::<PyPowerPreference>()?;
    m.add_class::<PyAlphaMode>()?;
    m.add_class::<PyAtmosphereMode>()?;
    m.add_class::<PyExtent3d>()?;
    m.add_class::<PyFace>()?;
    m.add_class::<PyTextureDimension>()?;
    m.add_class::<PyOpaqueRenderMethod>()?;
    m.add_class::<PyShader>()?;
    m.add_class::<PyShaderDefVal>()?;
    m.add_class::<PyShaderImport>()?;
    m.add_class::<PyShaderRef>()?;
    m.add_class::<PySource>()?;
    m.add_class::<PyTextureFormat>()?;
    m.add_class::<PyValidateShader>()?;
    m.add_class::<PyHdr>()?;
    m.add_class::<PyNoAutomaticBatching>()?;
    m.add_class::<PyNoIndirectDrawing>()?;
    m.add_class::<PyOcclusionCulling>()?;
    m.add_class::<PyTemporalJitter>()?;
    m.add_class::<PyMsaa>()?;
    m.add_class::<PyMipBias>()?;
    m.add_class::<PyColorGrading>()?;
    m.add_class::<PyColorGradingSection>()?;
    m.add_class::<PyColorGradingGlobal>()?;
    parent.add_submodule(&m)
}

// TODO: move to pybevy_shader crate
pub fn add_shader_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "shader")?;
    m.add_class::<PyShader>()?;
    m.add_class::<PyShaderDefVal>()?;
    m.add_class::<PyShaderImport>()?;
    m.add_class::<PyShaderRef>()?;
    m.add_class::<PySource>()?;
    m.add_class::<PyValidateShader>()?;
    parent.add_submodule(&m)
}

// TODO: move to pybevy_wgpu crate
pub fn add_wgpu_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "wgpu")?;
    m.add_class::<PyExtent3d>()?;
    m.add_class::<PyTextureDimension>()?;
    m.add_class::<PyTextureFormat>()?;
    parent.add_submodule(&m)
}
