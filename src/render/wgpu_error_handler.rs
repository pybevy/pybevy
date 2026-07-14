use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use bevy::{app::Plugin, render::renderer::RenderDevice};

/// What the uncaptured-error handler should do with a given wgpu error.
///
/// Pure extraction of the closure logic installed by `WgpuErrorHandlerPlugin`
/// so the dedup/classification can be unit-tested without a render device.
#[derive(Debug, PartialEq, Eq)]
enum WgpuErrorAction {
    /// Log the rendered error message once (first occurrence of a validation
    /// description).
    Log(String),
    /// Drop the error: an identical validation message was already logged.
    Drop,
    /// Fatal error (OutOfMemory/Internal): panic with the rendered message.
    Panic(String),
}

/// Classify a wgpu error and dedup validation messages against `seen`.
///
/// Validation errors are non-fatal and deduplicated by description: the first
/// occurrence yields [`WgpuErrorAction::Log`], repeats yield [`WgpuErrorAction::Drop`].
/// Every other error kind (OutOfMemory, Internal) yields [`WgpuErrorAction::Panic`].
fn classify_wgpu_error(error: &wgpu::Error, seen: &Mutex<HashSet<String>>) -> WgpuErrorAction {
    match error {
        wgpu::Error::Validation { description, .. } => {
            let is_new = seen
                .lock()
                .map(|mut set| set.insert(description.clone()))
                .unwrap_or(true);
            if is_new {
                WgpuErrorAction::Log(format!("{error}"))
            } else {
                WgpuErrorAction::Drop
            }
        }
        _ => WgpuErrorAction::Panic(format!("{error}")),
    }
}

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

        device
            .wgpu_device()
            .on_uncaptured_error(Arc::new(move |error| {
                match classify_wgpu_error(&error, &seen) {
                    WgpuErrorAction::Log(msg) => {
                        bevy::log::error!("wgpu validation error (non-fatal): {msg}");
                    }
                    WgpuErrorAction::Drop => {}
                    WgpuErrorAction::Panic(msg) => panic!("wgpu error: {msg}"),
                }
            }));
    }
}
