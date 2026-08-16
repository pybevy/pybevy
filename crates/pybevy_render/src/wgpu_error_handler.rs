use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy::{app::Plugin, prelude::Resource, render::renderer::RenderDevice};

/// Shared signal set after an uncaptured wgpu validation error.
///
/// Interactive hosts can pause frame submission while leaving their reload
/// transport alive, then clear the signal after loading a replacement scene.
#[derive(Clone, Default, Resource)]
pub struct WgpuValidationErrorState(Arc<AtomicBool>);

impl WgpuValidationErrorState {
    pub fn has_error(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn clear(&self) {
        self.0.store(false, Ordering::Release);
    }

    fn mark_error(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// What the uncaptured-error handler should do with a given wgpu error.
///
/// Pure extraction of the closure logic installed by [`WgpuErrorHandlerPlugin`]
/// so the deduplication and classification can be unit-tested without a render
/// device.
#[derive(Debug, PartialEq, Eq)]
enum WgpuErrorAction {
    /// Log the rendered error message once (first occurrence of a validation
    /// description).
    Log(String),
    /// Drop the error: an identical validation message was already logged.
    Drop,
    /// Fatal error (out of memory or internal): panic with the rendered message.
    Panic(String),
}

/// Classify a wgpu error and deduplicate validation messages against `seen`.
///
/// Validation errors invalidate the affected GPU operation, but do not poison
/// the device. Keeping them non-fatal is important for hot reload: a broken
/// scene can be replaced without terminating the app or Web Worker. Errors
/// that indicate device instability remain fatal.
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

/// Installs PyBevy's non-fatal handler for wgpu validation errors.
///
/// Bevy's default uncaptured-error callback forwards validation errors to its
/// render error state machine, whose default policy exits the application.
/// That is a poor fit for an interactive Python/hot-reload host: a bad asset or
/// scene should fail that frame, while leaving the runtime available to load a
/// corrected scene. This plugin replaces only the uncaptured-error callback.
/// Validation errors are logged once and discarded; out-of-memory and internal
/// errors still panic because continuing with an unstable device is unsafe.
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

        let seen: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        device
            .wgpu_device()
            .on_uncaptured_error(Arc::new(move |error| {
                if matches!(error, wgpu::Error::Validation { .. }) {
                    validation_state.mark_error();
                }
                match classify_wgpu_error(&error, &seen) {
                    WgpuErrorAction::Log(msg) => {
                        tracing::error!("wgpu validation error (non-fatal): {msg}");
                    }
                    WgpuErrorAction::Drop => {}
                    WgpuErrorAction::Panic(msg) => panic!("wgpu error: {msg}"),
                }
            }));
    }
}
