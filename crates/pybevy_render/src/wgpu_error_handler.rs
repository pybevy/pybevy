use std::sync::Arc;

use bevy::{app::Plugin, render::renderer::RenderDevice};
pub use pybevy_core::gpu_validation::WgpuValidationErrorState;
use pybevy_core::{gpu_validation::WgpuValidationEntry, public_error::wgpu_validation_log};

/// What the uncaptured-error handler should do with a given wgpu error.
///
/// Pure extraction of the closure logic installed by [`WgpuErrorHandlerPlugin`]
/// so the deduplication and classification can be unit-tested without a render
/// device.
#[derive(Debug, PartialEq, Eq)]
enum WgpuErrorAction {
    /// Log the rendered error message once (first occurrence since the last
    /// clear of the validation state).
    Log(String),
    /// Drop the error: an identical validation message was already logged.
    Drop,
    /// Fatal error (out of memory or internal): panic with the rendered message.
    Panic(String),
}

/// Classify a wgpu error, record validation errors in `state`, and deduplicate
/// their log lines.
///
/// Validation errors invalidate the affected GPU operation, but do not poison
/// the device. Keeping them non-fatal is important for hot reload: a broken
/// scene can be replaced without terminating the app or Web Worker. Errors
/// that indicate device instability remain fatal.
fn classify_wgpu_error(error: &wgpu::Error, state: &WgpuValidationErrorState) -> WgpuErrorAction {
    match error {
        wgpu::Error::Validation { .. } => {
            let entry = WgpuValidationEntry::from_message(format!("{error}"));
            if state.record(&entry) {
                WgpuErrorAction::Log(wgpu_validation_log(
                    entry.label.as_deref(),
                    entry.is_pipeline,
                    &entry.message,
                ))
            } else {
                WgpuErrorAction::Drop
            }
        }
        _ => WgpuErrorAction::Panic(format!("{error}")),
    }
}

/// Installs PyBevy's non-fatal handler for wgpu validation errors.
///
/// Bevy's default uncaptured-error callback forwards validation errors to its
/// render error state machine, whose default policy exits the application.
/// That is a poor fit for an interactive Python/hot-reload host: a bad asset or
/// scene should fail that frame, while leaving the runtime available to load a
/// corrected scene. This plugin replaces only the uncaptured-error callback.
/// Validation errors are recorded in [`WgpuValidationErrorState`] and logged
/// once per message; out-of-memory and internal errors still panic because
/// continuing with an unstable device is unsafe.
pub struct WgpuErrorHandlerPlugin;

impl Plugin for WgpuErrorHandlerPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.init_resource::<WgpuValidationErrorState>();
    }

    fn finish(&self, app: &mut bevy::app::App) {
        let validation_state = app.world().resource::<WgpuValidationErrorState>().clone();
        let Some(render_app) = app.get_sub_app(bevy::render::RenderApp) else {
            return;
        };

        let Some(device) = render_app.world().get_resource::<RenderDevice>() else {
            return;
        };

        device
            .wgpu_device()
            .on_uncaptured_error(Arc::new(move |error| {
                match classify_wgpu_error(&error, &validation_state) {
                    WgpuErrorAction::Log(msg) => tracing::error!("{msg}"),
                    WgpuErrorAction::Drop => {}
                    WgpuErrorAction::Panic(msg) => panic!("wgpu error: {msg}"),
                }
            }));
    }
}
