use bevy::{app::App, prelude::*, render::renderer::RenderDevice};
use pybevy_core::{
    PluginBuild, PyHandle,
    public_error::{
        READBACK_RENDER_DEVICE_MISSING, invalid_entity_bits, readback_camera_missing,
        readback_receiver_missing,
    },
};
use pybevy_macros::pyplugin;
use pybevy_render::readback::validate_readback_dimensions;
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};

use super::readback::{FrameReceiver, ImageCopier, ImageCopyPlugin, list_entities, poll_frame};
use crate::app::{app::PyApp, plugin::PyPlugin};

#[pyplugin(ImageCopyPlugin)]
#[pyclass(
    name = "ImageCopyPlugin",
    module = "pybevy._internal.render_readback",
    extends = PyPlugin
)]
pub struct PyImageCopyPlugin;

#[pymethods]
impl PyImageCopyPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyImageCopyPlugin, PyPlugin).into()
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(ImageCopyPlugin);
            Ok(())
        })
    }
}

impl PluginBuild for PyImageCopyPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(ImageCopyPlugin);
        Ok(())
    }
}

/// Spawn ImageCopier and FrameReceiver on a camera entity
/// This must be called AFTER the app has been initialized (after first update)
/// so that RenderDevice resource is available
#[pyfunction]
pub fn spawn_image_copier(
    app: &Bound<'_, PyApp>,
    camera_entity: u64,
    image_handle: &Bound<'_, PyHandle>,
    width: u32,
    height: u32,
) -> PyResult<()> {
    app.borrow().with_bevy_app(|bevy_app| {
        let handle: Handle<Image> = image_handle.extract::<PyHandle>()?.try_into()?;

        let entity = Entity::try_from_bits(camera_entity)
            .ok_or_else(|| PyValueError::new_err(invalid_entity_bits(camera_entity)))?;
        bevy_app
            .world()
            .get_entity(entity)
            .map_err(|_| PyRuntimeError::new_err(readback_camera_missing(entity)))?;

        let render_device = bevy_app
            .world()
            .get_resource::<RenderDevice>()
            .ok_or_else(|| PyRuntimeError::new_err(READBACK_RENDER_DEVICE_MISSING))?;
        validate_readback_dimensions(width, height, 4, render_device.limits().max_buffer_size)
            .map_err(PyValueError::new_err)?;
        let (frame_receiver, sender) = FrameReceiver::new(width, height);
        let copier = ImageCopier::new(handle, width, height, render_device, sender)
            .with_source_entity(entity);

        bevy_app
            .world_mut()
            .get_entity_mut(entity)
            .map_err(|_| PyRuntimeError::new_err(readback_camera_missing(entity)))?
            .insert((copier, frame_receiver));

        Ok(())
    })
}

/// Python wrapper for reading frames from a specific camera entity
#[pyfunction]
pub fn try_receive_frame(app: &Bound<'_, PyApp>, camera_entity: u64) -> PyResult<Option<Vec<u8>>> {
    app.borrow().with_bevy_app(|bevy_app| {
        let entity = Entity::try_from_bits(camera_entity)
            .ok_or_else(|| PyValueError::new_err(invalid_entity_bits(camera_entity)))?;

        let entity_ref = bevy_app
            .world()
            .get_entity(entity)
            .map_err(|_| PyRuntimeError::new_err(readback_camera_missing(entity)))?;
        let receiver = entity_ref
            .get::<FrameReceiver>()
            .ok_or_else(|| PyRuntimeError::new_err(readback_receiver_missing(entity)))?;

        Ok(receiver.try_recv())
    })
}

/// Poll the global readback frame buffer for the latest frame from a camera entity.
///
/// This does NOT require PyApp access: safe to call from within ECS systems.
/// The Rust-side `collect_readback_frames` system populates this buffer each frame.
///
/// Returns None if no frame is available yet.
#[pyfunction]
pub fn poll_readback_frame(entity_bits: u64) -> Option<Vec<u8>> {
    poll_frame(entity_bits)
}

/// List all entity bits that have readback frames available.
///
/// Useful for discovering which cameras are producing frames without
/// knowing their entity IDs in advance.
#[pyfunction]
pub fn list_readback_entities() -> Vec<u64> {
    list_entities()
}

pub(crate) fn add_to_module(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyImageCopyPlugin>()?;
    module.add_function(wrap_pyfunction!(spawn_image_copier, module)?)?;
    module.add_function(wrap_pyfunction!(try_receive_frame, module)?)?;
    module.add_function(wrap_pyfunction!(poll_readback_frame, module)?)?;
    module.add_function(wrap_pyfunction!(list_readback_entities, module)?)?;
    Ok(())
}
