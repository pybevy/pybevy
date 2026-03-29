pub mod ambient_light;
pub mod atmosphere_environment_map_light;
pub mod cascade;
pub mod cascade_shadow_config;
pub mod cascades;
pub mod clustered_decal;
pub mod directional_light;
pub mod environment_map_light;
pub mod fog_volume;
pub mod generated_environment_map_light;
pub mod irradiance_volume;
pub mod light_texture;
pub mod plugin;
pub mod point_light;
pub mod shadow_filtering_method;
pub mod shadow_map;
pub mod shadow_markers;
pub mod spot_light;
pub mod sun_disk;
pub mod volumetric_fog;

pub use ambient_light::{PyAmbientLight, PyGlobalAmbientLight};
pub use atmosphere_environment_map_light::PyAtmosphereEnvironmentMapLight;
use bevy::light::{
    AmbientLight, AtmosphereEnvironmentMapLight, CascadeShadowConfig, Cascades, ClusteredDecal,
    DirectionalLight, DirectionalLightShadowMap, DirectionalLightTexture, EnvironmentMapLight,
    FogVolume, GeneratedEnvironmentMapLight, GlobalAmbientLight, IrradianceVolume, LightProbe,
    NotShadowCaster, NotShadowReceiver, PointLight, PointLightShadowMap, PointLightTexture,
    ShadowFilteringMethod, SpotLight, SpotLightTexture, SunDisk, TransmittedShadowReceiver,
    VolumetricFog, VolumetricLight,
};
pub use cascade::PyCascade;
pub use cascade_shadow_config::PyCascadeShadowConfig;
pub use cascades::PyCascades;
pub use clustered_decal::PyClusteredDecal;
pub use directional_light::PyDirectionalLight;
pub use environment_map_light::PyEnvironmentMapLight;
pub use fog_volume::PyFogVolume as PyFogVolumeNew;
pub use generated_environment_map_light::PyGeneratedEnvironmentMapLight;
pub use irradiance_volume::PyIrradianceVolume;
pub use light_texture::{PyDirectionalLightTexture, PyPointLightTexture, PySpotLightTexture};
pub use plugin::PyLightPlugin;
pub use point_light::PyPointLight;
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{
    component_bridge, newtype_bridge, plugin_bridge, resource_bridge, unit_bridge,
};
use pyo3::prelude::*;
pub use shadow_filtering_method::PyShadowFilteringMethod;
pub use shadow_map::{PyDirectionalLightShadowMap, PyPointLightShadowMap};
pub use shadow_markers::{
    PyLightProbe, PyNotShadowCaster, PyNotShadowReceiver, PyTransmittedShadowReceiver,
    PyVolumetricLight,
};
pub use spot_light::PySpotLight;
pub use sun_disk::PySunDisk;
pub use volumetric_fog::PyVolumetricFog;

component_bridge!(
    PointLight,
    PyPointLight,
    view_fields = [
        intensity,
        range,
        radius,
        shadow_depth_bias,
        shadow_normal_bias,
        shadow_map_near_z,
        shadows_enabled,
        affects_lightmapped_mesh_diffuse
    ],
    batch_only_fields = [color]
);
component_bridge!(
    DirectionalLight,
    PyDirectionalLight,
    view_fields = [
        illuminance,
        shadow_depth_bias,
        shadow_normal_bias,
        shadows_enabled,
        affects_lightmapped_mesh_diffuse
    ],
    batch_only_fields = [color]
);
component_bridge!(
    SpotLight,
    PySpotLight,
    view_fields = [
        intensity,
        range,
        radius,
        shadow_depth_bias,
        shadow_normal_bias,
        shadow_map_near_z,
        outer_angle,
        inner_angle,
        shadows_enabled,
        affects_lightmapped_mesh_diffuse
    ],
    batch_only_fields = [color]
);

unit_bridge!(NotShadowCaster, PyNotShadowCaster);
unit_bridge!(NotShadowReceiver, PyNotShadowReceiver);
unit_bridge!(TransmittedShadowReceiver, PyTransmittedShadowReceiver);
unit_bridge!(VolumetricLight, PyVolumetricLight);
unit_bridge!(LightProbe, PyLightProbe);

newtype_bridge!(ShadowFilteringMethod, PyShadowFilteringMethod);

component_bridge!(SunDisk, PySunDisk, view_fields = [angular_size, intensity]);
component_bridge!(
    AtmosphereEnvironmentMapLight,
    PyAtmosphereEnvironmentMapLight,
    view_fields = [intensity, affects_lightmapped_mesh_diffuse]
);
component_bridge!(
    VolumetricFog,
    PyVolumetricFog,
    view_fields = [ambient_intensity, step_count, jitter],
    batch_only_fields = [ambient_color]
);

component_bridge!(
    EnvironmentMapLight,
    PyEnvironmentMapLight,
    view_fields = [intensity, affects_lightmapped_mesh_diffuse]
);
component_bridge!(
    GeneratedEnvironmentMapLight,
    PyGeneratedEnvironmentMapLight,
    view_fields = [intensity, affects_lightmapped_mesh_diffuse]
);
component_bridge!(DirectionalLightTexture, PyDirectionalLightTexture, view_only_fields = [tiled: bool]);
component_bridge!(SpotLightTexture, PySpotLightTexture);
component_bridge!(PointLightTexture, PyPointLightTexture);
component_bridge!(ClusteredDecal, PyClusteredDecal, view_fields = [tag]);
component_bridge!(
    FogVolume,
    PyFogVolumeNew,
    "FogVolume",
    view_fields = [
        density_factor,
        absorption,
        scattering,
        scattering_asymmetry,
        light_intensity
    ],
    batch_only_fields = [fog_color, light_tint]
);
component_bridge!(
    IrradianceVolume,
    PyIrradianceVolume,
    view_fields = [intensity, affects_lightmapped_meshes]
);
component_bridge!(Cascades, PyCascades, no_insert);
component_bridge!(
    AmbientLight,
    PyAmbientLight,
    view_fields = [brightness, affects_lightmapped_meshes],
    batch_only_fields = [color]
);
component_bridge!(
    CascadeShadowConfig,
    PyCascadeShadowConfig,
    view_fields = [overlap_proportion, minimum_distance]
);

plugin_bridge!(PyLightPlugin, bevy::light::LightPlugin);

resource_bridge!(GlobalAmbientLight, PyGlobalAmbientLight);
resource_bridge!(DirectionalLightShadowMap, PyDirectionalLightShadowMap);
resource_bridge!(PointLightShadowMap, PyPointLightShadowMap);
pub fn register_light_bridges() {
    global_registry::register_component_bridge(PointLightBridge);
    global_registry::register_component_bridge(DirectionalLightBridge);
    global_registry::register_component_bridge(SpotLightBridge);

    register_point_light_batch();
    register_directional_light_batch();
    register_spot_light_batch();
    register_sun_disk_batch();
    register_volumetric_fog_batch();
    register_environment_map_light_batch();
    register_atmosphere_environment_map_light_batch();
    register_generated_environment_map_light_batch();
    register_clustered_decal_batch();
    register_irradiance_volume_batch();
    register_ambient_light_batch();
    register_cascade_shadow_config_batch();
    register_fog_volume_batch();

    global_registry::register_component_bridge(NotShadowCasterBridge);
    global_registry::register_component_bridge(NotShadowReceiverBridge);
    global_registry::register_component_bridge(TransmittedShadowReceiverBridge);
    global_registry::register_component_bridge(VolumetricLightBridge);
    global_registry::register_component_bridge(LightProbeBridge);
    global_registry::register_component_bridge(ShadowFilteringMethodBridge);
    global_registry::register_component_bridge(SunDiskBridge);
    global_registry::register_component_bridge(AtmosphereEnvironmentMapLightBridge);
    global_registry::register_component_bridge(VolumetricFogBridge);
    global_registry::register_component_bridge(EnvironmentMapLightBridge);
    global_registry::register_component_bridge(GeneratedEnvironmentMapLightBridge);
    global_registry::register_component_bridge(DirectionalLightTextureBridge);
    global_registry::register_component_bridge(SpotLightTextureBridge);
    global_registry::register_component_bridge(PointLightTextureBridge);
    global_registry::register_component_bridge(ClusteredDecalBridge);
    global_registry::register_component_bridge(FogVolumeBridge);
    global_registry::register_component_bridge(IrradianceVolumeBridge);
    global_registry::register_component_bridge(CascadesBridge);
    global_registry::register_component_bridge(AmbientLightBridge);
    global_registry::register_component_bridge(CascadeShadowConfigBridge);

    global_registry::register_resource_bridge(GlobalAmbientLightBridge);
    global_registry::register_resource_bridge(DirectionalLightShadowMapBridge);
    global_registry::register_resource_bridge(PointLightShadowMapBridge);

    plugin_registry::register_plugin_bridge(LightPluginBridge);
}
pub fn add_light_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_light_bridges();

    m.add_class::<PyLightPlugin>()?;
    m.add_class::<PyPointLight>()?;
    m.add_class::<PyDirectionalLight>()?;
    m.add_class::<PySpotLight>()?;
    m.add_class::<PyNotShadowCaster>()?;
    m.add_class::<PyNotShadowReceiver>()?;
    m.add_class::<PyTransmittedShadowReceiver>()?;
    m.add_class::<PyVolumetricLight>()?;
    m.add_class::<PyLightProbe>()?;
    m.add_class::<PyShadowFilteringMethod>()?;
    m.add_class::<PySunDisk>()?;
    m.add_class::<PyAtmosphereEnvironmentMapLight>()?;
    m.add_class::<PyVolumetricFog>()?;
    m.add_class::<PyCascade>()?;

    m.add_class::<PyEnvironmentMapLight>()?;
    m.add_class::<PyGeneratedEnvironmentMapLight>()?;
    m.add_class::<PyDirectionalLightTexture>()?;
    m.add_class::<PySpotLightTexture>()?;
    m.add_class::<PyPointLightTexture>()?;
    m.add_class::<PyClusteredDecal>()?;
    m.add_class::<PyFogVolumeNew>()?;
    m.add_class::<PyIrradianceVolume>()?;
    m.add_class::<PyCascades>()?;
    m.add_class::<PyAmbientLight>()?;
    m.add_class::<PyGlobalAmbientLight>()?;
    m.add_class::<PyPointLightShadowMap>()?;
    m.add_class::<PyDirectionalLightShadowMap>()?;
    m.add_class::<PyCascadeShadowConfig>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "light")?;
    add_light_classes(&m)?;
    parent.add_submodule(&m)
}
