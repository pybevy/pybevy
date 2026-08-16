pub mod ambient_light;
pub mod atmosphere;
pub mod atmosphere_environment_map_light;
pub mod cascade;
pub mod cascade_shadow_config;
pub mod cascades;
pub mod clustered_decal;
pub mod directional_light;
pub mod environment_map_light;
pub mod falloff;
pub mod fog_volume;
pub mod generated_environment_map_light;
pub mod gizmos;
pub mod irradiance_volume;
pub mod light_texture;
pub mod parallax_correction;
pub mod phase_function;
pub mod plugin;
pub mod point_light;
pub mod rect_light;
pub mod scattering_medium;
pub mod scattering_term;
pub mod scattering_terms;
pub mod shadow_filtering_method;
pub mod shadow_map;
pub mod shadow_markers;
pub mod skybox;
pub mod spot_light;
pub mod sun_disk;
pub mod volumetric_fog;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        ambient_light::{PyAmbientLight, PyGlobalAmbientLight},
        directional_light::PyDirectionalLight,
        environment_map_light::PyEnvironmentMapLight,
        generated_environment_map_light::PyGeneratedEnvironmentMapLight,
        gizmos::{PyLightGizmoColor, PyLightGizmoConfigGroup, PyShowLightGizmo},
        parallax_correction::PyParallaxCorrection,
        plugin::PyLightPlugin,
        point_light::PyPointLight,
        rect_light::PyRectLight,
        skybox::PySkybox,
        spot_light::PySpotLight,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "light")?;
    m.add_class::<plugin::PyLightPlugin>()?;
    m.add_class::<gizmos::PyLightGizmoColor>()?;
    m.add_class::<gizmos::PyLightGizmoConfigGroup>()?;
    m.add_class::<gizmos::PyShowLightGizmo>()?;
    m.add_class::<point_light::PyPointLight>()?;
    m.add_class::<directional_light::PyDirectionalLight>()?;
    m.add_class::<spot_light::PySpotLight>()?;
    m.add_class::<rect_light::PyRectLight>()?;
    m.add_class::<parallax_correction::PyParallaxCorrection>()?;
    parallax_correction::register_parallax_correction_variants(&m)?;
    m.add_class::<shadow_markers::PyNotShadowCaster>()?;
    m.add_class::<shadow_markers::PyNotShadowReceiver>()?;
    m.add_class::<shadow_markers::PyTransmittedShadowReceiver>()?;
    m.add_class::<shadow_markers::PyVolumetricLight>()?;
    m.add_class::<shadow_markers::PyLightProbe>()?;
    m.add_class::<shadow_filtering_method::PyShadowFilteringMethod>()?;
    m.add_class::<sun_disk::PySunDisk>()?;
    m.add_class::<atmosphere_environment_map_light::PyAtmosphereEnvironmentMapLight>()?;
    m.add_class::<volumetric_fog::PyVolumetricFog>()?;
    m.add_class::<cascade::PyCascade>()?;

    m.add_class::<environment_map_light::PyEnvironmentMapLight>()?;
    m.add_class::<generated_environment_map_light::PyGeneratedEnvironmentMapLight>()?;
    m.add_class::<light_texture::PyDirectionalLightTexture>()?;
    m.add_class::<light_texture::PySpotLightTexture>()?;
    m.add_class::<light_texture::PyPointLightTexture>()?;
    m.add_class::<clustered_decal::PyClusteredDecal>()?;
    m.add_class::<fog_volume::PyFogVolume>()?;
    m.add_class::<irradiance_volume::PyIrradianceVolume>()?;
    m.add_class::<cascades::PyCascades>()?;
    m.add_class::<ambient_light::PyAmbientLight>()?;
    m.add_class::<ambient_light::PyGlobalAmbientLight>()?;
    m.add_class::<shadow_map::PyPointLightShadowMap>()?;
    m.add_class::<shadow_map::PyDirectionalLightShadowMap>()?;
    m.add_class::<cascade_shadow_config::PyCascadeShadowConfig>()?;
    m.add_class::<atmosphere::PyAtmosphere>()?;
    m.add_class::<scattering_medium::PyScatteringMedium>()?;
    m.add_class::<scattering_term::PyScatteringTerm>()?;
    m.add_class::<scattering_terms::PyScatteringTerms>()?;
    m.add_class::<phase_function::PyPhaseFunction>()?;
    phase_function::register_phase_function_variants(&m)?;
    m.add_class::<falloff::PyFalloff>()?;
    falloff::register_falloff_variants(&m)?;
    m.add_class::<skybox::PySkybox>()?;
    parent.add_submodule(&m)
}
