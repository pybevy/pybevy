pub mod cache_cleanup;
pub mod color_grading;
pub mod color_grading_component;
pub mod extent3d;
pub mod face;
pub mod mip_bias;
pub mod msaa;
pub mod pipeline_drain;
pub mod plugin;
pub mod power_preference;
pub mod readback;
pub mod render_creation;
pub mod shader_buffer;
pub mod temporal_jitter;
pub mod texture_aspect;
pub mod texture_descriptor;
pub mod texture_dimension;
pub mod texture_format;
pub mod texture_usages;
pub mod texture_view_descriptor;
pub mod texture_view_dimension;
pub mod tonemapping;
pub mod unit_markers;
pub mod vertex_format;
pub mod wgpu_error_handler;
pub mod wgpu_settings;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        color_grading::{PyColorGradingGlobal, PyColorGradingSection},
        color_grading_component::PyColorGrading,
        extent3d::PyExtent3d,
        mip_bias::PyMipBias,
        msaa::PyMsaa,
        plugin::PyRenderPlugin,
        temporal_jitter::PyTemporalJitter,
        texture_dimension::PyTextureDimension,
        texture_format::PyTextureFormat,
        texture_usages::PyTextureUsages,
        tonemapping::PyTonemapping,
        vertex_format::PyVertexFormat,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "render")?;
    m.add_class::<plugin::PyRenderPlugin>()?;
    m.add_class::<plugin::PyTexturePlugin>()?;
    m.add_class::<power_preference::PyPowerPreference>()?;
    m.add_class::<render_creation::PyRenderCreation>()?;
    render_creation::register_render_creation_variants(&m)?;
    m.add_class::<wgpu_settings::PyWgpuSettings>()?;
    m.add_class::<wgpu_settings::PyBackends>()?;
    m.add_class::<wgpu_settings::PyWgpuSettingsPriority>()?;
    m.add_class::<wgpu_settings::PyGles3MinorVersion>()?;
    m.add_class::<face::PyFace>()?;
    m.add_class::<unit_markers::PyNoAutomaticBatching>()?;
    m.add_class::<unit_markers::PyNoIndirectDrawing>()?;
    m.add_class::<unit_markers::PyOcclusionCulling>()?;
    m.add_class::<temporal_jitter::PyTemporalJitter>()?;
    m.add_class::<extent3d::PyExtent3d>()?;
    m.add_class::<texture_dimension::PyTextureDimension>()?;
    m.add_class::<texture_format::PyTextureFormat>()?;
    m.add_class::<texture_usages::PyTextureUsages>()?;
    m.add_class::<texture_descriptor::PyTextureDescriptor>()?;
    m.add_class::<texture_aspect::PyTextureAspect>()?;
    m.add_class::<texture_view_descriptor::PyTextureViewDescriptor>()?;
    m.add_class::<texture_view_dimension::PyTextureViewDimension>()?;
    m.add_class::<vertex_format::PyVertexFormat>()?;
    m.add_class::<msaa::PyMsaa>()?;
    m.add_class::<mip_bias::PyMipBias>()?;
    m.add_class::<color_grading_component::PyColorGrading>()?;
    m.add_class::<color_grading::PyColorGradingSection>()?;
    m.add_class::<color_grading::PyColorGradingGlobal>()?;
    m.add_class::<shader_buffer::PyShaderBuffer>()?;
    m.add_class::<tonemapping::PyTonemapping>()?;
    parent.add_submodule(&m)
}
