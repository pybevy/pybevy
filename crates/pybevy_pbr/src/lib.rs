pub mod atmosphere;
pub mod atmosphere_settings;
pub mod default_opaque_renderer_method;
pub mod distance_fog;
pub mod falloff;
pub mod fog_falloff;
pub mod forward_decal;
pub mod lightmap;
pub mod no_wireframe;
pub mod opaque_renderer_method;
pub mod parallax_mapping_method;
pub mod plugin;
pub mod scattering_medium;
pub mod screen_space_ambient_occlusion;
pub mod screen_space_reflections;
pub mod shader_material;
pub mod shader_material_py;
pub mod ssao_quality_level;
pub mod standard_material;
pub mod uv_channel;
pub mod wireframe;
pub mod wireframe_color;
pub mod wireframe_config;
pub mod wireframe_material;

pub use atmosphere::PyAtmosphere;
pub use atmosphere_settings::PyAtmosphereSettings;
use bevy::pbr::{
    Atmosphere, AtmosphereSettings, DefaultOpaqueRendererMethod, DistanceFog, Lightmap,
    MeshMaterial3d, ScatteringMedium, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections,
    StandardMaterial,
    decal::ForwardDecal,
    wireframe::{NoWireframe, Wireframe, WireframeColor, WireframeConfig, WireframeMaterial},
};
pub use default_opaque_renderer_method::PyDefaultOpaqueRendererMethod;
pub use distance_fog::PyDistanceFog;
pub use falloff::PyFalloff;
pub use fog_falloff::PyFogFalloff;
pub use forward_decal::PyForwardDecal;
pub use lightmap::PyLightmap;
pub use no_wireframe::PyNoWireframe;
pub use opaque_renderer_method::PyOpaqueRendererMethod;
pub use parallax_mapping_method::PyParallaxMappingMethod;
pub use plugin::PyPbrPlugin;
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{
    asset_bridge, component_bridge, handle_bridge, plugin_bridge, resource_bridge, unit_bridge,
};
use pyo3::prelude::*;
pub use scattering_medium::PyScatteringMedium;
pub use screen_space_ambient_occlusion::PyScreenSpaceAmbientOcclusion;
pub use screen_space_reflections::PyScreenSpaceReflections;
use shader_material::ShaderMaterial;
pub use shader_material_py::{PyMeshMaterial3dShader, PyShaderMaterial, PyShaderMaterialPlugin};
pub use ssao_quality_level::PyScreenSpaceAmbientOcclusionQualityLevel;
pub use standard_material::PyStandardMaterial;
pub use uv_channel::PyUvChannel;
pub use wireframe::PyWireframe;
pub use wireframe_color::PyWireframeColor;
pub use wireframe_config::PyWireframeConfig;
pub use wireframe_material::PyWireframeMaterial;

component_bridge!(
    DistanceFog,
    PyDistanceFog,
    view_fields = [directional_light_exponent],
    batch_only_fields = [color, directional_light_color]
);
component_bridge!(
    ScreenSpaceAmbientOcclusion,
    PyScreenSpaceAmbientOcclusion,
    view_fields = [constant_object_thickness]
);
component_bridge!(
    ScreenSpaceReflections,
    PyScreenSpaceReflections,
    view_fields = [
        perceptual_roughness_threshold,
        thickness,
        linear_steps,
        linear_march_exponent,
        bisection_steps,
        use_secant
    ]
);
component_bridge!(Atmosphere, PyAtmosphere, view_only_fields = [bottom_radius: f32, top_radius: f32]);
component_bridge!(
    AtmosphereSettings,
    PyAtmosphereSettings,
    view_fields = [
        transmittance_lut_samples,
        multiscattering_lut_dirs,
        multiscattering_lut_samples,
        sky_view_lut_samples,
        aerial_view_lut_samples,
        aerial_view_lut_max_distance,
        scene_units_to_m,
        sky_max_samples
    ]
);
component_bridge!(Wireframe, PyWireframe);
component_bridge!(
    WireframeColor,
    PyWireframeColor,
    batch_only_fields = [color]
);
component_bridge!(NoWireframe, PyNoWireframe);
component_bridge!(Lightmap, PyLightmap);
unit_bridge!(ForwardDecal, PyForwardDecal);

plugin_bridge!(PyPbrPlugin, bevy::pbr::PbrPlugin);

resource_bridge!(WireframeConfig, PyWireframeConfig);
resource_bridge!(DefaultOpaqueRendererMethod, PyDefaultOpaqueRendererMethod);

asset_bridge!(StandardMaterial, PyStandardMaterial);
asset_bridge!(WireframeMaterial, PyWireframeMaterial, not_loadable);
asset_bridge!(ScatteringMedium, PyScatteringMedium, not_loadable);
asset_bridge!(ShaderMaterial, PyShaderMaterial, not_loadable);

handle_bridge!(
    MeshMaterial3d::<ShaderMaterial>,
    PyMeshMaterial3dShader,
    "MeshMaterial3dShader"
);

plugin_bridge!(
    PyShaderMaterialPlugin,
    bevy::pbr::MaterialPlugin::<ShaderMaterial>,
    |_py_plugin: &pyo3::Bound<'_, pyo3::PyAny>, app: &mut bevy::app::App| {
        // Clear stale handles from previous app runs in the same process
        shader_material::clear_shader_registries();
        app.add_plugins(bevy::pbr::MaterialPlugin::<ShaderMaterial>::default());
        app.add_systems(bevy::app::Last, shader_material::sync_shader_handles);
        Ok(())
    }
);
pub fn register_pbr_bridges() {
    global_registry::register_component_bridge(DistanceFogBridge);
    global_registry::register_component_bridge(ScreenSpaceAmbientOcclusionBridge);
    global_registry::register_component_bridge(ScreenSpaceReflectionsBridge);
    global_registry::register_component_bridge(AtmosphereBridge);
    global_registry::register_component_bridge(AtmosphereSettingsBridge);
    global_registry::register_component_bridge(WireframeBridge);
    global_registry::register_component_bridge(WireframeColorBridge);
    global_registry::register_component_bridge(NoWireframeBridge);
    global_registry::register_component_bridge(LightmapBridge);
    global_registry::register_component_bridge(ForwardDecalBridge);
    register_distance_fog_batch();
    register_screen_space_ambient_occlusion_batch();
    register_screen_space_reflections_batch();
    register_atmosphere_settings_batch();
    register_wireframe_color_batch();

    plugin_registry::register_plugin_bridge(PbrPluginBridge);

    global_registry::register_resource_bridge(WireframeConfigBridge);
    global_registry::register_resource_bridge(DefaultOpaqueRendererMethodBridge);

    global_registry::register_asset_bridge(StandardMaterialBridge);
    global_registry::register_asset_bridge(WireframeMaterialBridge);
    global_registry::register_asset_bridge(ScatteringMediumBridge);
    global_registry::register_asset_bridge(ShaderMaterialBridge);

    global_registry::register_component_bridge(MeshMaterial3dShaderBridge);
    plugin_registry::register_plugin_bridge(MaterialPluginBridge);
}
pub fn add_pbr_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_pbr_bridges();

    m.add_class::<PyPbrPlugin>()?;
    m.add_class::<PyDistanceFog>()?;
    m.add_class::<PyFogFalloff>()?;
    m.add_class::<PyScreenSpaceAmbientOcclusion>()?;
    m.add_class::<PyScreenSpaceAmbientOcclusionQualityLevel>()?;
    m.add_class::<PyScreenSpaceReflections>()?;
    m.add_class::<PyAtmosphere>()?;
    m.add_class::<PyAtmosphereSettings>()?;
    m.add_class::<PyParallaxMappingMethod>()?;
    m.add_class::<PyStandardMaterial>()?;
    m.add_class::<PyUvChannel>()?;
    m.add_class::<PyWireframe>()?;
    m.add_class::<PyWireframeColor>()?;
    m.add_class::<PyNoWireframe>()?;
    m.add_class::<PyWireframeConfig>()?;
    m.add_class::<PyWireframeMaterial>()?;
    m.add_class::<PyScatteringMedium>()?;
    m.add_class::<PyDefaultOpaqueRendererMethod>()?;
    m.add_class::<PyLightmap>()?;
    m.add_class::<PyForwardDecal>()?;
    m.add_class::<PyFalloff>()?;
    m.add_class::<PyOpaqueRendererMethod>()?;

    m.add_class::<PyShaderMaterial>()?;
    m.add_class::<PyShaderMaterialPlugin>()?;
    m.add_class::<PyMeshMaterial3dShader>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "pbr")?;
    add_pbr_classes(&m)?;
    // Render types re-exported through the pbr Python module
    m.add_class::<pybevy_render::PyOpaqueRenderMethod>()?;
    m.add_class::<pybevy_render::PyAtmosphereMode>()?;
    parent.add_submodule(&m)
}
