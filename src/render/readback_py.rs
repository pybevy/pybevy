use bevy::{prelude::*, render::renderer::RenderDevice};
use pybevy_core::{PyHandle, plugin::plugin_registry};
use pybevy_macros::plugin_bridge;
use pyo3::prelude::*;

use super::readback::{FrameReceiver, ImageCopier, ImageCopyPlugin};
use crate::app::{app::PyApp, plugin::PyPlugin};

#[pyclass(name = "ImageCopyPlugin", extends = PyPlugin)]
pub struct PyImageCopyPlugin;

#[pymethods]
impl PyImageCopyPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyImageCopyPlugin, PyPlugin)
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(ImageCopyPlugin);
            Ok(())
        })
    }
}

plugin_bridge!(PyImageCopyPlugin, ImageCopyPlugin, |_py_plugin, app| {
    app.add_plugins(ImageCopyPlugin);
    Ok(())
});

pub(crate) fn register_readback_bridges() {
    plugin_registry::register_plugin_bridge(ImageCopyPluginBridge);
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

        let render_device = bevy_app
            .world()
            .get_resource::<RenderDevice>()
            .ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    "RenderDevice not found - has the app been updated at least once?",
                )
            })?;

        let (frame_receiver, sender) = FrameReceiver::new(width, height);
        let copier = ImageCopier::new(handle, width, height, render_device, sender);
        let entity = Entity::from_bits(camera_entity);

        bevy_app
            .world_mut()
            .entity_mut(entity)
            .insert((copier, frame_receiver));

        Ok(())
    })
}

/// Python wrapper for reading frames from a specific camera entity
#[pyfunction]
pub fn try_receive_frame(app: &Bound<'_, PyApp>, camera_entity: u64) -> PyResult<Option<Vec<u8>>> {
    app.borrow().with_bevy_app(|bevy_app| {
        let entity = Entity::from_bits(camera_entity);

        let entity_ref = bevy_app.world().get_entity(entity).map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Camera entity {:?} not found",
                entity
            ))
        })?;

        let receiver = entity_ref.get::<FrameReceiver>().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "FrameReceiver not found on camera entity {:?} - is ImageCopier spawned?",
                entity
            ))
        })?;

        Ok(receiver.try_recv())
    })
}

/// Poll the global readback frame buffer for the latest frame from a camera entity.
///
/// This does NOT require PyApp access — safe to call from within ECS systems.
/// The Rust-side `collect_readback_frames` system populates this buffer each frame.
///
/// Returns None if no frame is available yet.
#[pyfunction]
pub fn poll_readback_frame(entity_bits: u64) -> Option<Vec<u8>> {
    super::readback::poll_frame(entity_bits)
}

/// List all entity bits that have readback frames available.
///
/// Useful for discovering which cameras are producing frames without
/// knowing their entity IDs in advance.
#[pyfunction]
pub fn list_readback_entities() -> Vec<u64> {
    super::readback::list_entities()
}

pub(crate) fn add_to_module(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyImageCopyPlugin>()?;
    module.add_function(wrap_pyfunction!(spawn_image_copier, module)?)?;
    module.add_function(wrap_pyfunction!(try_receive_frame, module)?)?;
    module.add_function(wrap_pyfunction!(poll_readback_frame, module)?)?;
    module.add_function(wrap_pyfunction!(list_readback_entities, module)?)?;
    Ok(())
}
