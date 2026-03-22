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
use bevy::render::{
    batching::NoAutomaticBatching,
    camera::{MipBias, TemporalJitter},
    experimental::occlusion_culling::OcclusionCulling,
    view::{ColorGrading, Hdr, Msaa, NoIndirectDrawing},
};
pub use color_grading::{PyColorGradingGlobal, PyColorGradingSection};
pub use color_grading_component::PyColorGrading;
pub use face::PyFace;
pub use mip_bias::PyMipBias;
pub use msaa::PyMsaa;
pub use opaque_render_method::PyOpaqueRenderMethod;
pub use plugin::PyRenderPlugin;
pub use power_preference::PyPowerPreference;
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
// Re-export image sampler types from pybevy_image for backward compatibility
pub use pybevy_image::{PyImageAddressMode, PyImageFilterMode, PyImageSampler};
use pybevy_macros::{component_bridge, newtype_bridge, plugin_bridge, unit_bridge};
// Re-export shader types from pybevy_shader for backward compatibility
pub use pybevy_shader::{
    PyShader, PyShaderDefVal, PyShaderImport, PyShaderRef, PySource, PyValidateShader,
};
// Re-export wgpu types from pybevy_wgpu for backward compatibility
pub use pybevy_wgpu::{PyExtent3d, PyTextureDimension, PyTextureFormat};
use pyo3::prelude::*;
pub use temporal_jitter::PyTemporalJitter;
pub use unit_markers::{PyHdr, PyNoAutomaticBatching, PyNoIndirectDrawing, PyOcclusionCulling};

// Generate bridges for unit/marker components
unit_bridge!(Hdr, PyHdr);
unit_bridge!(NoAutomaticBatching, PyNoAutomaticBatching);
unit_bridge!(NoIndirectDrawing, PyNoIndirectDrawing);
unit_bridge!(OcclusionCulling, PyOcclusionCulling);

// Generate bridges for ComponentStorage-based components
component_bridge!(TemporalJitter, PyTemporalJitter);
component_bridge!(ColorGrading, PyColorGrading);

// Generate bridges for newtype/enum components
newtype_bridge!(Msaa, PyMsaa, copy);
newtype_bridge!(MipBias, PyMipBias);

plugin_bridge!(
    PyRenderPlugin,
    bevy::render::RenderPlugin,
    |py_plugin, app| {
        let config: pyo3::PyRef<'_, PyRenderPlugin> = py_plugin.extract()?;
        let mut wgpu_settings = bevy::render::settings::WgpuSettings::default();
        if let Some(ref pp) = config.power_preference {
            wgpu_settings.power_preference = (*pp).into();
        }
        let mut render_plugin = bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(wgpu_settings),
            ..Default::default()
        };
        if let Some(sync) = config.synchronous_pipeline_compilation {
            render_plugin.synchronous_pipeline_compilation = sync;
        }
        app.add_plugins(render_plugin);
        Ok(())
    }
);

pub fn register_render_bridges() {
    pybevy_shader::register_shader_bridges();
    plugin_registry::register_plugin_bridge(RenderPluginBridge);

    // Unit markers
    global_registry::register_component_bridge(HdrBridge);
    global_registry::register_component_bridge(NoAutomaticBatchingBridge);
    global_registry::register_component_bridge(NoIndirectDrawingBridge);
    global_registry::register_component_bridge(OcclusionCullingBridge);

    // ComponentStorage-based components
    global_registry::register_component_bridge(TemporalJitterBridge);
    global_registry::register_component_bridge(ColorGradingBridge);

    // Newtype/enum components
    global_registry::register_component_bridge(MsaaBridge);
    global_registry::register_component_bridge(MipBiasBridge);
}

pub fn add_render_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_render_bridges();

    m.add_class::<PyRenderPlugin>()?;
    m.add_class::<PyPowerPreference>()?;

    m.add_class::<PyAlphaMode>()?;
    m.add_class::<PyAtmosphereMode>()?;
    m.add_class::<PyExtent3d>()?;
    m.add_class::<PyFace>()?;
    m.add_class::<PyTextureDimension>()?;
    m.add_class::<PyImageFilterMode>()?;
    m.add_class::<PyImageAddressMode>()?;
    m.add_class::<PyImageSampler>()?;
    m.add_class::<PyOpaqueRenderMethod>()?;
    m.add_class::<PyShader>()?;
    m.add_class::<PyShaderDefVal>()?;
    m.add_class::<PyShaderImport>()?;
    m.add_class::<PyShaderRef>()?;
    m.add_class::<PySource>()?;
    m.add_class::<PyTextureFormat>()?;
    m.add_class::<PyValidateShader>()?;

    // Unit markers (moved from pybevy_camera)
    m.add_class::<PyHdr>()?;
    m.add_class::<PyNoAutomaticBatching>()?;
    m.add_class::<PyNoIndirectDrawing>()?;
    m.add_class::<PyOcclusionCulling>()?;

    // ComponentStorage-based components (moved from pybevy_camera)
    m.add_class::<PyTemporalJitter>()?;

    // Newtype/enum components (moved from pybevy_camera)
    m.add_class::<PyMsaa>()?;
    m.add_class::<PyMipBias>()?;

    // ComponentStorage-based components (moved from pybevy_camera)
    m.add_class::<PyColorGrading>()?;

    // Supporting types (moved from pybevy_camera)
    m.add_class::<PyColorGradingSection>()?;
    m.add_class::<PyColorGradingGlobal>()?;

    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "render")?;
    add_render_classes(&m)?;
    parent.add_submodule(&m)
}

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

pub fn add_wgpu_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "wgpu")?;
    add_render_classes(&m)?;
    parent.add_submodule(&m)
}
