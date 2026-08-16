pub mod atmosphere_mode;
pub mod atmosphere_settings;
pub mod contact_shadows;
pub mod default_opaque_renderer_method;
pub mod distance_fog;
pub mod fog_falloff;
pub mod forward_decal;
pub mod lightmap;
pub mod mesh3d_wireframe;
pub mod no_wireframe;
pub mod parallax_mapping_method;
pub mod plugin;
pub mod screen_space_ambient_occlusion;
pub mod screen_space_reflections;
pub mod screen_space_transmission;
pub mod screen_space_transmission_quality;
pub mod shader_material;
pub mod shader_material_py;
pub mod ssao_quality_level;
pub mod standard_material;
pub mod wireframe;
pub mod wireframe_color;
pub mod wireframe_config;
pub mod wireframe_line_width;
pub mod wireframe_material;
pub mod wireframe_plugin;
pub mod wireframe_topology;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        distance_fog::PyDistanceFog, fog_falloff::PyFogFalloff,
        parallax_mapping_method::PyParallaxMappingMethod, plugin::PyPbrPlugin,
        standard_material::PyStandardMaterial,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "pbr")?;
    m.add_class::<pybevy_core::PyMaterial>()?;
    m.add_class::<plugin::PyPbrPlugin>()?;
    m.add_class::<distance_fog::PyDistanceFog>()?;
    m.add_class::<fog_falloff::PyFogFalloff>()?;
    m.add_class::<screen_space_ambient_occlusion::PyScreenSpaceAmbientOcclusion>()?;
    m.add_class::<ssao_quality_level::PyScreenSpaceAmbientOcclusionQualityLevel>()?;
    m.add_class::<screen_space_reflections::PyScreenSpaceReflections>()?;
    m.add_class::<screen_space_transmission::PyScreenSpaceTransmission>()?;
    m.add_class::<screen_space_transmission_quality::PyScreenSpaceTransmissionQuality>()?;
    m.add_class::<contact_shadows::PyContactShadows>()?;
    m.add_class::<atmosphere_settings::PyAtmosphereSettings>()?;
    m.add_class::<parallax_mapping_method::PyParallaxMappingMethod>()?;
    m.add_class::<standard_material::PyStandardMaterial>()?;
    m.add_class::<wireframe::PyWireframe>()?;
    m.add_class::<wireframe_color::PyWireframeColor>()?;
    m.add_class::<no_wireframe::PyNoWireframe>()?;
    m.add_class::<wireframe_config::PyWireframeConfig>()?;
    m.add_class::<wireframe_line_width::PyWireframeLineWidth>()?;
    m.add_class::<wireframe_material::PyWireframeMaterial>()?;
    m.add_class::<mesh3d_wireframe::PyMesh3dWireframe>()?;
    m.add_class::<wireframe_topology::PyWireframeTopology>()?;
    m.add_class::<wireframe_plugin::PyWireframePlugin>()?;
    m.add_class::<default_opaque_renderer_method::PyDefaultOpaqueRendererMethod>()?;
    m.add_class::<lightmap::PyLightmap>()?;
    m.add_class::<forward_decal::PyForwardDecal>()?;
    m.add_class::<forward_decal::PyForwardDecalMaterialExt>()?;
    m.add_class::<forward_decal::PyForwardDecalMaterial>()?;
    m.add_class::<forward_decal::PyMeshMaterial3dForwardDecal>()?;
    m.add_class::<shader_material_py::PyShaderMaterial>()?;
    m.add_class::<shader_material_py::PyShaderMaterialPlugin>()?;
    m.add_class::<shader_material_py::PyMeshMaterial3dShader>()?;
    m.add_class::<atmosphere_mode::PyAtmosphereMode>()?;
    parent.add_submodule(&m)
}
