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

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "light")?;
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
    parent.add_submodule(&m)
}
