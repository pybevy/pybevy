pub mod aabb;
pub mod bloom;
pub mod bloom_composite_mode;
pub mod bloom_prefilter;
pub mod camera;
pub mod camera_3d;
pub mod camera_3d_depth_load_op;
pub mod camera_3d_depth_texture_usage;
pub mod camera_main_texture_usages;
pub mod clear_color;
pub mod clear_color_config;
pub mod core_pipeline_plugin;
pub mod cubemap_frusta;
pub mod cubemap_layout;
pub mod cubemap_visible_entities;
pub mod culling_sphere;
pub mod exposure;
pub mod frustum;
pub mod half_space;
pub mod inherited_visibility;
pub mod main_pass_resolution_override;
pub mod normalized_render_target;
pub mod physical_camera_parameters;
pub mod plugin;
pub mod projection;
pub mod render_layers;
pub mod render_target;
pub mod screen_space_transmission_quality;
pub mod skybox;
pub mod sub_camera_view;
pub mod tonemapping;
pub mod unit_markers;
pub mod view_visibility;
pub mod viewport;
pub mod visibility;
pub mod visibility_batch;
pub mod visibility_class;
pub mod visibility_range;
pub mod visible_mesh_entities;

pub use aabb::PyAabb;
use bevy::{
    camera::{
        Camera, Camera2d, Camera3d, CameraMainTextureUsages, ClearColor, Exposure, Projection,
        RenderTarget,
        primitives::{Aabb, CubemapFrusta, Frustum},
        visibility::{
            CubemapVisibleEntities, NoCpuCulling, NoFrustumCulling, RenderLayers, Visibility,
            VisibilityClass, VisibilityRange, VisibleMeshEntities,
        },
    },
    core_pipeline::{
        Skybox,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass},
        tonemapping::Tonemapping,
    },
    post_process::bloom::Bloom,
    prelude::{InheritedVisibility, ViewVisibility},
};
pub use bloom::PyBloom;
pub use bloom_composite_mode::PyBloomCompositeMode;
pub use bloom_prefilter::PyBloomPrefilter;
pub use camera::PyCamera;
pub use camera_3d::PyCamera3d;
pub use camera_3d_depth_load_op::PyCamera3dDepthLoadOp;
pub use camera_3d_depth_texture_usage::PyCamera3dDepthTextureUsage;
pub use camera_main_texture_usages::PyCameraMainTextureUsages;
pub use clear_color::PyClearColor;
pub use clear_color_config::PyClearColorConfig;
pub use core_pipeline_plugin::PyCorePipelinePlugin;
pub use cubemap_frusta::PyCubemapFrusta;
pub use cubemap_layout::PyCubemapLayout;
pub use cubemap_visible_entities::PyCubemapVisibleEntities;
pub use culling_sphere::PyCullingSphere;
pub use exposure::PyExposure;
pub use frustum::PyFrustum;
pub use half_space::PyHalfSpace;
pub use inherited_visibility::PyInheritedVisibility;
pub use main_pass_resolution_override::PyMainPassResolutionOverride;
pub use normalized_render_target::PyNormalizedRenderTarget;
pub use physical_camera_parameters::PyPhysicalCameraParameters;
pub use plugin::PyCameraPlugin;
pub use projection::{PyOrthographicProjection, PyPerspectiveProjection, PyProjection};
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{
    component_bridge, newtype_bridge, plugin_bridge, resource_bridge, unit_bridge,
};
// Re-export moved types from pybevy_render for backward compatibility
pub use pybevy_render::{
    PyColorGrading, PyColorGradingGlobal, PyColorGradingSection, PyHdr, PyMipBias, PyMsaa,
    PyNoAutomaticBatching, PyNoIndirectDrawing, PyOcclusionCulling, PyTemporalJitter,
};
// Re-export moved type from pybevy_sprite for backward compatibility
pub mod scaling_mode;
use pyo3::prelude::*;
pub use render_layers::PyRenderLayers;
pub use render_target::PyRenderTarget;
pub use scaling_mode::PyScalingMode;
pub use screen_space_transmission_quality::PyScreenSpaceTransmissionQuality;
pub use skybox::PySkybox;
pub use sub_camera_view::PySubCameraView;
pub use tonemapping::PyTonemapping;
pub use unit_markers::{
    PyCamera2d, PyDeferredPrepass, PyDepthPrepass, PyMotionVectorPrepass, PyNoCpuCulling,
    PyNoFrustumCulling, PyNormalPrepass,
};
pub use view_visibility::PyViewVisibility;
pub use viewport::PyViewport;
pub use visibility::PyVisibility;
pub use visibility_class::PyVisibilityClass;
pub use visibility_range::PyVisibilityRange;
pub use visible_mesh_entities::PyVisibleMeshEntities;

// Generate bridges for unit/marker components
unit_bridge!(NoCpuCulling, PyNoCpuCulling);
unit_bridge!(NoFrustumCulling, PyNoFrustumCulling);
unit_bridge!(Camera2d, PyCamera2d);
unit_bridge!(DepthPrepass, PyDepthPrepass);
unit_bridge!(NormalPrepass, PyNormalPrepass);
unit_bridge!(MotionVectorPrepass, PyMotionVectorPrepass);
unit_bridge!(DeferredPrepass, PyDeferredPrepass);

// Generate bridges for ComponentStorage-based components
component_bridge!(Camera, PyCamera, view_fields = [is_active]);
component_bridge!(Camera3d, PyCamera3d);
component_bridge!(InheritedVisibility, PyInheritedVisibility);
component_bridge!(ViewVisibility, PyViewVisibility);
component_bridge!(VisibilityRange, PyVisibilityRange, view_fields = [use_aabb]);
component_bridge!(Exposure, PyExposure, view_fields = [ev100]);
component_bridge!(RenderLayers, PyRenderLayers);
component_bridge!(Aabb, PyAabb);
component_bridge!(
    Bloom,
    PyBloom,
    view_fields = [
        intensity,
        low_frequency_boost,
        low_frequency_boost_curvature,
        high_pass_frequency,
        max_mip_dimension
    ]
);
component_bridge!(CubemapFrusta, PyCubemapFrusta);
component_bridge!(CubemapVisibleEntities, PyCubemapVisibleEntities);
component_bridge!(Frustum, PyFrustum);
component_bridge!(Projection, PyProjection);
component_bridge!(Visibility, PyVisibility);
component_bridge!(VisibilityClass, PyVisibilityClass);
component_bridge!(VisibleMeshEntities, PyVisibleMeshEntities);
component_bridge!(Skybox, PySkybox, view_fields = [brightness]);
component_bridge!(RenderTarget, PyRenderTarget);

// Generate bridges for newtype/enum components
newtype_bridge!(Tonemapping, PyTonemapping);
newtype_bridge!(CameraMainTextureUsages, PyCameraMainTextureUsages);
// Note: MainPassResolutionOverride doesn't impl Clone, so we can't use newtype_bridge

// Generate bridges for resources
resource_bridge!(ClearColor, PyClearColor);

// Generate plugin bridges via macro
plugin_bridge!(PyCameraPlugin, bevy::camera::CameraPlugin);
plugin_bridge!(
    PyCorePipelinePlugin,
    bevy::core_pipeline::CorePipelinePlugin
);
pub fn register_camera_bridges() {
    // Unit markers use dynamic dispatch
    global_registry::register_component_bridge(NoCpuCullingBridge);
    global_registry::register_component_bridge(NoFrustumCullingBridge);
    global_registry::register_component_bridge(Camera2dBridge);
    global_registry::register_component_bridge(DepthPrepassBridge);
    global_registry::register_component_bridge(NormalPrepassBridge);
    global_registry::register_component_bridge(MotionVectorPrepassBridge);
    global_registry::register_component_bridge(DeferredPrepassBridge);

    // ComponentStorage-based components
    global_registry::register_component_bridge(CameraBridge);
    global_registry::register_component_bridge(Camera3dBridge);
    global_registry::register_component_bridge(InheritedVisibilityBridge);
    global_registry::register_component_bridge(ViewVisibilityBridge);
    global_registry::register_component_bridge(VisibilityRangeBridge);
    global_registry::register_component_bridge(ExposureBridge);
    global_registry::register_component_bridge(RenderLayersBridge);
    global_registry::register_component_bridge(AabbBridge);
    global_registry::register_component_bridge(BloomBridge);
    global_registry::register_component_bridge(CubemapFrustaBridge);
    global_registry::register_component_bridge(CubemapVisibleEntitiesBridge);
    global_registry::register_component_bridge(FrustumBridge);
    global_registry::register_component_bridge(ProjectionBridge);
    global_registry::register_component_bridge(VisibilityBridge);
    global_registry::register_component_bridge(VisibilityClassBridge);
    global_registry::register_component_bridge(VisibleMeshEntitiesBridge);
    global_registry::register_component_bridge(SkyboxBridge);
    global_registry::register_component_bridge(RenderTargetBridge);

    // Newtype/enum components
    global_registry::register_component_bridge(TonemappingBridge);
    global_registry::register_component_bridge(CameraMainTextureUsagesBridge);
    // Note: MainPassResolutionOverride doesn't impl Clone, so no bridge registration

    // Resources
    global_registry::register_resource_bridge(ClearColorBridge);

    // Plugins
    plugin_registry::register_plugin_bridge(CameraPluginBridge);
    plugin_registry::register_plugin_bridge(CorePipelinePluginBridge);

    // Batch components
    visibility_batch::register_visibility_batch_bridge();
    register_exposure_batch();
    register_camera_batch();
    register_skybox_batch();
    register_visibility_range_batch();
    register_bloom_batch();
}
pub fn add_camera_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_camera_bridges();

    // Plugins
    m.add_class::<PyCameraPlugin>()?;
    m.add_class::<PyCorePipelinePlugin>()?;

    // Unit markers (camera-specific, defined here)
    m.add_class::<PyNoCpuCulling>()?;
    m.add_class::<PyNoFrustumCulling>()?;
    m.add_class::<PyCamera2d>()?;
    m.add_class::<PyDepthPrepass>()?;
    m.add_class::<PyNormalPrepass>()?;
    m.add_class::<PyMotionVectorPrepass>()?;
    m.add_class::<PyDeferredPrepass>()?;

    // Unit markers (re-exported from pybevy_render for backward compatibility)
    m.add_class::<PyHdr>()?;
    m.add_class::<PyNoAutomaticBatching>()?;
    m.add_class::<PyNoIndirectDrawing>()?;
    m.add_class::<PyOcclusionCulling>()?;

    // ComponentStorage-based components
    m.add_class::<PyCamera>()?;
    m.add_class::<PyCamera3d>()?;
    m.add_class::<PyInheritedVisibility>()?;
    m.add_class::<PyViewVisibility>()?;
    m.add_class::<PyVisibilityRange>()?;
    m.add_class::<PyExposure>()?;
    m.add_class::<PyRenderLayers>()?;
    m.add_class::<PyAabb>()?;
    m.add_class::<PyBloom>()?;
    m.add_class::<PyColorGrading>()?;
    m.add_class::<PyCubemapFrusta>()?;
    m.add_class::<PyCubemapVisibleEntities>()?;
    m.add_class::<PyFrustum>()?;
    m.add_class::<PyProjection>()?;
    m.add_class::<PyVisibility>()?;
    m.add_class::<visibility_batch::PyVisibilityBatch>()?;
    m.add_class::<PyVisibilityClass>()?;
    m.add_class::<PyVisibleMeshEntities>()?;
    m.add_class::<PySkybox>()?;

    // ComponentStorage-based (re-exported from pybevy_render for backward compatibility)
    m.add_class::<PyTemporalJitter>()?;

    // RenderTarget types
    m.add_class::<PyRenderTarget>()?;
    m.add_class::<PyNormalizedRenderTarget>()?;

    // Supporting types
    m.add_class::<PyPhysicalCameraParameters>()?;
    m.add_class::<PyHalfSpace>()?;
    m.add_class::<PyCullingSphere>()?;
    m.add_class::<PyScalingMode>()?;
    m.add_class::<PyCubemapLayout>()?;
    m.add_class::<PyScreenSpaceTransmissionQuality>()?;
    m.add_class::<PyBloomCompositeMode>()?;
    m.add_class::<PyBloomPrefilter>()?;
    m.add_class::<PyCamera3dDepthLoadOp>()?;
    m.add_class::<PyCamera3dDepthTextureUsage>()?;
    m.add_class::<PyClearColorConfig>()?;
    m.add_class::<PySubCameraView>()?;
    m.add_class::<PyPerspectiveProjection>()?;
    m.add_class::<PyOrthographicProjection>()?;

    // Supporting types (re-exported from pybevy_render for backward compatibility)
    m.add_class::<PyColorGradingSection>()?;
    m.add_class::<PyColorGradingGlobal>()?;

    // Newtype/enum components
    m.add_class::<PyTonemapping>()?;
    m.add_class::<PyCameraMainTextureUsages>()?;
    m.add_class::<PyMainPassResolutionOverride>()?;
    m.add_class::<PyViewport>()?;

    // Newtype/enum (re-exported from pybevy_render for backward compatibility)
    m.add_class::<PyMsaa>()?;
    m.add_class::<PyMipBias>()?;

    // Resources
    m.add_class::<PyClearColor>()?;

    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "camera")?;
    add_camera_classes(&m)?;
    parent.add_submodule(&m)
}

pub fn add_core_pipeline_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "core_pipeline")?;
    m.add_class::<PyCorePipelinePlugin>()?;
    parent.add_submodule(&m)
}
