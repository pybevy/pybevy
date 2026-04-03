use std::sync::Arc;

use bevy::{
    app::{App, First, Last, Plugin},
    prelude::{IntoScheduleConfigs, Resource},
};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

use crate::{
    api_index::ApiIndex,
    bridge::{self, SharedLatestError, SharedLatestErrorResource, SseEventBroadcaster},
    server::{AppState, ServerConfig},
};

#[pyplugin(ControlBevyPlugin)]
#[pyclass(name = "McpPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone)]
pub struct PyControlPlugin {
    pub port: u16,
    pub host: String,
    pub screenshot: bool,
    pub manipulation: bool,
    pub execute_python: bool,
    pub api_discovery: bool,
}

#[pymethods]
impl PyControlPlugin {
    #[new]
    #[pyo3(signature = (
        port = 8420,
        host = "127.0.0.1".to_string(),
        screenshot = true,
        manipulation = true,
        execute_python = false,
        api_discovery = true,
    ))]
    pub fn new(
        port: u16,
        host: String,
        screenshot: bool,
        manipulation: bool,
        execute_python: bool,
        api_discovery: bool,
    ) -> (Self, PyPlugin) {
        (
            PyControlPlugin {
                port,
                host,
                screenshot,
                manipulation,
                execute_python,
                api_discovery,
            },
            PyPlugin,
        )
    }

    pub fn __repr__(&self) -> String {
        format!(
            "McpPlugin(port={}, host='{}', screenshot={}, manipulation={}, execute_python={}, api_discovery={})",
            self.port,
            self.host,
            self.screenshot,
            self.manipulation,
            self.execute_python,
            self.api_discovery,
        )
    }
}

impl Default for PyControlPlugin {
    fn default() -> Self {
        PyControlPlugin {
            port: 8420,
            host: "127.0.0.1".to_string(),
            screenshot: true,
            manipulation: true,
            execute_python: false,
            api_discovery: true,
        }
    }
}

impl PluginBuild for PyControlPlugin {
    fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        let config: PyRef<'_, PyControlPlugin> = py_plugin.extract()?;
        app.add_plugins(ControlBevyPlugin {
            config: config.clone(),
        });
        Ok(())
    }
}

/// Bevy resource storing the control server config (for hot reload preservation)
#[derive(Resource, Clone)]
pub struct ControlConfig {
    pub port: u16,
    pub host: String,
    pub screenshot: bool,
    pub manipulation: bool,
    pub execute_python: bool,
    pub api_discovery: bool,
}

/// The actual Bevy plugin that sets up the HTTP control server
pub struct ControlBevyPlugin {
    pub config: PyControlPlugin,
}

impl Plugin for ControlBevyPlugin {
    fn build(&self, app: &mut App) {
        // Guard against double-registration
        if app.world().contains_resource::<ControlConfig>() {
            return;
        }

        let config = &self.config;

        // Build API index from .pyi stubs
        let api_index = if config.api_discovery {
            let pybevy_dir = Python::attach(|py| {
                let pybevy = py.import("pybevy").ok()?;
                let path_list = pybevy.getattr("__path__").ok()?;
                let first: String = path_list.get_item(0).ok()?.extract().ok()?;
                let path = std::path::PathBuf::from(first);
                path.is_dir().then_some(path)
            });
            match pybevy_dir {
                Some(dir) => ApiIndex::build(&dir),
                None => {
                    bevy::log::warn!(
                        "[Control] Could not find pybevy package directory for API stubs"
                    );
                    ApiIndex::build(std::path::Path::new(""))
                }
            }
        } else {
            ApiIndex::build(std::path::Path::new(""))
        };

        // Create the channel
        let (sender, receiver) = bridge::create_channel();

        // Create SSE broadcaster
        let sse_broadcaster = SseEventBroadcaster::new();

        // Create shared error state
        let shared_error = SharedLatestError::default();

        let server_config = ServerConfig {
            screenshot_enabled: config.screenshot,
            manipulation_enabled: config.manipulation,
            execute_python_enabled: config.execute_python,
            api_discovery_enabled: config.api_discovery,
        };

        // Create shared schedule registry
        let schedule_registry = crate::handlers::schedule::SharedScheduleRegistry::default();

        // Build server state
        let server_state = AppState::new(
            sender,
            sse_broadcaster.clone(),
            Arc::new(api_index),
            server_config,
            shared_error.clone(),
            schedule_registry.clone(),
        );

        // Insert Bevy resources
        app.insert_resource(receiver);
        app.insert_resource(sse_broadcaster);
        app.insert_resource(SharedLatestErrorResource(shared_error));
        app.init_resource::<crate::handlers::schedule::ActiveSchedules>();
        app.insert_resource(crate::handlers::schedule::SharedScheduleRegistryResource(
            schedule_registry,
        ));
        app.insert_resource(ControlConfig {
            port: config.port,
            host: config.host.clone(),
            screenshot: config.screenshot,
            manipulation: config.manipulation,
            execute_python: config.execute_python,
            api_discovery: config.api_discovery,
        });

        app.insert_non_send_resource(Box::new(crate::runtime_pyo3::Pyo3ControlRuntime)
            as Box<dyn crate::runtime::ControlRuntime>);

        // Ensure the app keeps ticking when unfocused so MCP requests are processed
        #[cfg(feature = "bevy_winit")]
        app.add_systems(bevy::app::Startup, configure_mcp_update_mode);

        // Register the exclusive poll system in First schedule
        app.add_systems(First, bridge::control_poll_system);
        // Schedule processor runs after control_poll_system (also in First)
        app.add_systems(
            First,
            crate::handlers::schedule::process_active_schedules.after(bridge::control_poll_system),
        );

        // Register screenshot and reload processing systems (runs after render)
        app.add_systems(
            Last,
            (
                crate::handlers::screenshot::process_pending_screenshots,
                crate::handlers::screenshot::process_pending_timelines,
                crate::handlers::reload::process_pending_reloads,
                crate::handlers::reload::process_pending_reload_and_capture,
                crate::handlers::turnaround::process_pending_turnarounds,
            ),
        );

        // Register global observer for screenshot capture completion
        app.add_observer(crate::handlers::screenshot::screenshot_captured_observer);
        app.init_resource::<crate::handlers::screenshot::PendingScreenshotResponders>();
        app.init_resource::<crate::handlers::screenshot::PendingTimelines>();
        app.init_resource::<crate::handlers::screenshot::TimelineCaptures>();
        app.init_resource::<crate::handlers::turnaround::PendingTurnarounds>();
        app.init_resource::<crate::handlers::turnaround::TurnaroundCaptures>();

        // Start the HTTP server
        crate::server::start_server(config.host.clone(), config.port, server_state);
    }
}

/// Sets `WinitSettings::unfocused_mode` to `Reactive` with a short wait so that
/// MCP/control requests (screenshots, turnarounds, timelines) still run when the
/// window is not focused, without spinning at uncapped FPS.
#[cfg(feature = "bevy_winit")]
fn configure_mcp_update_mode(settings: Option<bevy::prelude::ResMut<bevy::winit::WinitSettings>>) {
    use bevy::winit::UpdateMode;

    let Some(mut settings) = settings else {
        return;
    };
    // Reactive wakes on input/window/device events AND after the wait duration,
    // so MCP requests that trigger redraws are handled promptly while idle
    // frames are capped to ~60fps instead of spinning continuously.
    let target = UpdateMode::reactive(std::time::Duration::from_millis(16));
    if matches!(settings.unfocused_mode, UpdateMode::Reactive { .. }) {
        return;
    }
    settings.unfocused_mode = target;
    bevy::log::info!("[Control] Set unfocused_mode to Reactive(16ms) for MCP responsiveness");
}
