use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use bevy::{app::Plugin, render::renderer::RenderDevice};

/// Plugin that installs a non-panicking wgpu error handler for validation errors.
///
/// By default, wgpu's uncaptured error handler panics, which kills the render
/// thread on shader validation errors (e.g., binding mismatches). This plugin
/// replaces it with a handler that logs validation errors instead, deduplicating
/// repeated messages. OutOfMemory and Internal errors still panic.
///
/// This is needed for hot reload to work and not crash the pybevy renderer
pub struct WgpuErrorHandlerPlugin;

impl Plugin for WgpuErrorHandlerPlugin {
    fn build(&self, _app: &mut bevy::app::App) {}

    fn finish(&self, app: &mut bevy::app::App) {
        let Some(render_app) = app.get_sub_app(bevy::render::RenderApp) else {
            return;
        };

        let Some(device) = render_app.world().get_resource::<RenderDevice>() else {
            return;
        };

        let seen: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        device.wgpu_device().on_uncaptured_error(Arc::new(move |error| {
            match &error {
                wgpu::Error::Validation { description, .. } => {
                    let is_new = seen
                        .lock()
                        .map(|mut set| set.insert(description.clone()))
                        .unwrap_or(true);
                    if is_new {
                        bevy::log::error!("wgpu validation error (non-fatal): {error}");
                    }
                }
                _ => panic!("wgpu error: {error}"),
            }
        }));
    }
}
