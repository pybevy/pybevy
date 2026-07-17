pub mod aabb;
pub mod camera;
pub mod camera_3d;
pub mod camera_3d_depth_load_op;
pub mod camera_3d_depth_texture_usage;
pub mod camera_main_texture_usages;
pub mod clear_color;
pub mod clear_color_config;
pub mod cubemap_frusta;
pub mod cubemap_layout;
pub mod cubemap_visible_entities;
pub mod exposure;
pub mod frustum;
pub mod image_render_target;
pub mod inherited_visibility;
pub mod main_pass_resolution_override;
pub mod manual_texture_view_handle;
pub mod msaa_writeback;
pub mod normalized_render_target;
pub mod physical_camera_parameters;
pub mod plugin;
pub mod projection;
pub mod render_layers;
pub mod render_target;
pub mod sphere;
pub mod sub_camera_view;
pub mod unit_markers;
pub mod view_visibility;
pub mod viewport;
pub mod visibility;
pub mod visibility_batch;
pub mod visibility_class;
pub mod visibility_range;
pub mod visible_mesh_entities;

pub mod scaling_mode;
use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        camera::PyCamera,
        camera_3d::PyCamera3d,
        clear_color::PyClearColor,
        clear_color_config::PyClearColorConfig,
        exposure::PyExposure,
        image_render_target::PyImageRenderTarget,
        inherited_visibility::PyInheritedVisibility,
        manual_texture_view_handle::PyManualTextureViewHandle,
        msaa_writeback::PyMsaaWriteback,
        plugin::PyCameraPlugin,
        projection::{PyOrthographicProjection, PyPerspectiveProjection, PyProjection},
        render_layers::PyRenderLayers,
        scaling_mode::PyScalingMode,
        unit_markers::PyCamera2d,
        view_visibility::PyViewVisibility,
        viewport::PyViewport,
        visibility::PyVisibility,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    visibility_batch::register_visibility_batch_bridge();

    let m = PyModule::new(parent.py(), "camera")?;
    m.add_class::<plugin::PyCameraPlugin>()?;
    m.add_class::<unit_markers::PyNoCpuCulling>()?;
    m.add_class::<unit_markers::PyNoFrustumCulling>()?;
    m.add_class::<unit_markers::PyCamera2d>()?;
    m.add_class::<unit_markers::PyDepthPrepass>()?;
    m.add_class::<unit_markers::PyNormalPrepass>()?;
    m.add_class::<unit_markers::PyMotionVectorPrepass>()?;
    m.add_class::<unit_markers::PyDeferredPrepass>()?;
    m.add_class::<camera::PyCamera>()?;
    m.add_class::<msaa_writeback::PyMsaaWriteback>()?;
    m.add_class::<camera_3d::PyCamera3d>()?;
    m.add_class::<inherited_visibility::PyInheritedVisibility>()?;
    m.add_class::<view_visibility::PyViewVisibility>()?;
    m.add_class::<visibility_range::PyVisibilityRange>()?;
    m.add_class::<exposure::PyExposure>()?;
    m.add_class::<render_layers::PyRenderLayers>()?;
    m.add_class::<aabb::PyAabb>()?;
    m.add_class::<cubemap_frusta::PyCubemapFrusta>()?;
    m.add_class::<cubemap_visible_entities::PyCubemapVisibleEntities>()?;
    m.add_class::<frustum::PyFrustum>()?;
    m.add_class::<projection::PyProjection>()?;
    projection::register_projection_variants(&m)?;
    m.add_class::<visibility::PyVisibility>()?;
    m.add_class::<visibility_batch::PyVisibilityBatch>()?;
    m.add_class::<visibility_class::PyVisibilityClass>()?;
    m.add_class::<visible_mesh_entities::PyVisibleMeshEntities>()?;
    m.add_class::<render_target::PyRenderTarget>()?;
    render_target::register_render_target_variants(&m)?;
    m.add_class::<normalized_render_target::PyNormalizedRenderTarget>()?;
    m.add_class::<image_render_target::PyImageRenderTarget>()?;
    m.add_class::<manual_texture_view_handle::PyManualTextureViewHandle>()?;
    m.add_class::<physical_camera_parameters::PyPhysicalCameraParameters>()?;
    m.add_class::<sphere::PySphere>()?;
    m.add_class::<scaling_mode::PyScalingMode>()?;
    m.add_class::<cubemap_layout::PyCubemapLayout>()?;
    m.add_class::<camera_3d_depth_load_op::PyCamera3dDepthLoadOp>()?;
    m.add_class::<camera_3d_depth_texture_usage::PyCamera3dDepthTextureUsage>()?;
    m.add_class::<clear_color_config::PyClearColorConfig>()?;
    m.add_class::<sub_camera_view::PySubCameraView>()?;
    m.add_class::<projection::PyPerspectiveProjection>()?;
    m.add_class::<projection::PyOrthographicProjection>()?;
    m.add_class::<camera_main_texture_usages::PyCameraMainTextureUsages>()?;
    m.add_class::<main_pass_resolution_override::PyMainPassResolutionOverride>()?;
    m.add_class::<viewport::PyViewport>()?;
    m.add_class::<clear_color::PyClearColor>()?;
    parent.add_submodule(&m)
}
