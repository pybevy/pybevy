#[cfg(feature = "pyo3-runtime")]
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(feature = "bevy_winit")]
use std::time::Duration;

#[cfg(feature = "pyo3-runtime")]
use bevy::log::warn;
#[cfg(feature = "bevy_winit")]
use bevy::{
    app::Startup,
    log::info,
    prelude::ResMut,
    winit::{UpdateMode, WinitSettings},
};
use bevy::{
    app::{App, First, Last, MainScheduleOrder, Plugin},
    asset::UntypedAssetLoadFailedEvent,
    ecs::{message::Messages, schedule::ScheduleLabel},
    prelude::{IntoScheduleConfigs, Res, Resource, resource_exists},
    render::{Render, RenderApp, RenderSystems},
};
#[cfg(feature = "pyo3-runtime")]
use pybevy_core::{PluginBuild, PyPlugin, ensure_asset_access_registry, repr::str_repr};
#[cfg(feature = "pyo3-runtime")]
use pybevy_macros::pyplugin;
#[cfg(feature = "pyo3-runtime")]
use pyo3::prelude::*;

#[cfg(feature = "pyo3-runtime")]
use crate::pyo3::runtime::Pyo3ControlRuntime;
use crate::{
    asset_load_errors::{AssetLoadFailures, record_asset_load_failures},
    asset_reference::AssetReferenceSession,
    capture::{
        analysis::CapturedFrames,
        camera::hold_capture_pose,
        depth::process_pending_depth_samples,
        readiness::{RenderFrameReadiness, update_render_frame_readiness},
        screenshot::{
            PendingScreenshotResponders, process_pending_screenshots, screenshot_captured_observer,
        },
        shader_errors::{
            ShaderErrors, UnresolvedShaderImports, publish_shader_errors,
            track_unresolved_shader_imports,
        },
        timeline::{PendingTimelines, TimelineCaptures, process_pending_timelines},
        turnaround::{PendingTurnarounds, TurnaroundCaptures, process_pending_turnarounds},
    },
    discovery::ApiIndex,
    engine::{
        channel::{ControlSender, FrameHeartbeat, create_channel},
        poll::control_poll_system,
        runtime::ControlRuntime,
    },
    reload::process_pending_reloads,
    schedule::{
        ActiveSchedules, SharedScheduleRegistry, SharedScheduleRegistryResource,
        process_active_schedules,
    },
    transport::{
        server::{AppState, start_server},
        sse::SseEventBroadcaster,
    },
};

#[cfg(feature = "pyo3-runtime")]
#[pyplugin(ControlBevyPlugin)]
#[pyclass(name = "McpPlugin", module = "pybevy.mcp", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyControlPlugin {
    pub port: u16,
    pub host: String,
    pub screenshot: bool,
    pub manipulation: bool,
    pub execute_python: bool,
    pub api_discovery: bool,
    pub token: Option<String>,
}

#[cfg(feature = "pyo3-runtime")]
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
        token = None,
    ))]
    pub fn new(
        port: u16,
        host: String,
        screenshot: bool,
        manipulation: bool,
        execute_python: bool,
        api_discovery: bool,
        token: Option<String>,
    ) -> PyClassInitializer<Self> {
        (
            PyControlPlugin {
                port,
                host,
                screenshot,
                manipulation,
                execute_python,
                api_discovery,
                token,
            },
            PyPlugin,
        )
            .into()
    }

    pub fn __repr__(&self) -> String {
        let flag = |value: bool| if value { "True" } else { "False" };
        format!(
            "McpPlugin(port={}, host={}, screenshot={}, manipulation={}, execute_python={}, api_discovery={})",
            self.port,
            str_repr(&self.host),
            flag(self.screenshot),
            flag(self.manipulation),
            flag(self.execute_python),
            flag(self.api_discovery),
        )
    }
}

#[cfg(feature = "pyo3-runtime")]
impl Default for PyControlPlugin {
    fn default() -> Self {
        PyControlPlugin {
            port: 8420,
            host: "127.0.0.1".to_string(),
            screenshot: true,
            manipulation: true,
            execute_python: false,
            api_discovery: true,
            token: None,
        }
    }
}

#[cfg(feature = "pyo3-runtime")]
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
    /// Bearer token every request except `/health` must carry, when set.
    pub token: Option<String>,
}

/// Runs after `Last`, so a frame counts as finished only once every user system has returned.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct ControlFrameEnd;

fn mark_frame_started(heartbeat: Res<FrameHeartbeat>) {
    heartbeat.mark_started();
}

fn mark_frame_finished(heartbeat: Res<FrameHeartbeat>) {
    heartbeat.mark_finished();
}

/// Installs the control channel, schedule state and per-frame systems that
/// serve requests through `runtime`, and returns the request sender.
pub fn install_control_runtime(
    app: &mut App,
    config: ControlConfig,
    runtime: Box<dyn ControlRuntime>,
) -> (ControlSender, SharedScheduleRegistry) {
    let (sender, receiver) = create_channel();
    let exclusive_execution = sender.exclusive_execution();
    let schedule_registry = SharedScheduleRegistry::default();

    app.insert_resource(receiver);
    app.insert_resource(exclusive_execution);
    app.insert_resource(sender.heartbeat());
    app.init_schedule(ControlFrameEnd);
    app.world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(Last, ControlFrameEnd);
    app.add_systems(First, mark_frame_started.before(control_poll_system));
    app.add_systems(ControlFrameEnd, mark_frame_finished);
    app.init_resource::<ActiveSchedules>();
    app.insert_resource(SharedScheduleRegistryResource(schedule_registry.clone()));
    app.init_resource::<AssetReferenceSession>();
    app.init_resource::<AssetLoadFailures>();
    app.add_systems(
        Last,
        record_asset_load_failures.run_if(resource_exists::<Messages<UntypedAssetLoadFailedEvent>>),
    );
    app.insert_resource(config);
    app.insert_non_send(runtime);

    // Ensure the app keeps ticking when unfocused so MCP requests are processed
    #[cfg(feature = "bevy_winit")]
    app.add_systems(Startup, configure_mcp_update_mode);

    app.add_systems(First, control_poll_system);
    app.add_systems(First, process_active_schedules.after(control_poll_system));
    app.add_systems(
        Last,
        (
            hold_capture_pose,
            process_pending_screenshots,
            process_pending_timelines,
            process_pending_reloads,
            process_pending_turnarounds,
            process_pending_depth_samples,
        ),
    );
    app.add_observer(screenshot_captured_observer);
    app.init_resource::<CapturedFrames>();
    app.init_resource::<PendingScreenshotResponders>();
    app.init_resource::<PendingTimelines>();
    app.init_resource::<TimelineCaptures>();
    app.init_resource::<PendingTurnarounds>();
    app.init_resource::<TurnaroundCaptures>();

    (sender, schedule_registry)
}

/// Installs the control runtime and starts the HTTP server. A repeat call is
/// a no-op, so the server never binds its port twice.
pub fn build_control_plugin(
    app: &mut App,
    config: ControlConfig,
    api_index: ApiIndex,
    runtime: Box<dyn ControlRuntime>,
) {
    if app.world().contains_resource::<ControlConfig>() {
        return;
    }
    let sse_broadcaster = SseEventBroadcaster::new();
    app.insert_resource(sse_broadcaster.clone());
    let (sender, schedule_registry) = install_control_runtime(app, config.clone(), runtime);
    let server_state = AppState::new(
        sender,
        sse_broadcaster,
        Arc::new(api_index),
        config.clone(),
        schedule_registry,
    );
    start_server(config.host, config.port, server_state);
}

/// Shares render-frame completion and current shader compile errors between
/// the render world and the main world.
pub fn install_render_frame_readiness(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let readiness = RenderFrameReadiness::default();
    let shader_errors = ShaderErrors::default();
    render_app
        .insert_resource(readiness.clone())
        .insert_resource(shader_errors.clone())
        .add_systems(
            Render,
            (update_render_frame_readiness, publish_shader_errors).after(RenderSystems::Render),
        );
    app.insert_resource(readiness);
    app.insert_resource(shader_errors);
    app.init_resource::<UnresolvedShaderImports>();
    app.add_systems(Last, track_unresolved_shader_imports);
}

/// Adds render-frame readiness once the render sub-app exists.
pub struct RenderFrameReadinessPlugin;

impl Plugin for RenderFrameReadinessPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        install_render_frame_readiness(app);
    }
}

/// The PyO3 control plugin: the neutral control plugin served by `Pyo3ControlRuntime`.
#[cfg(feature = "pyo3-runtime")]
pub struct ControlBevyPlugin {
    pub config: PyControlPlugin,
}

#[cfg(feature = "pyo3-runtime")]
impl Plugin for ControlBevyPlugin {
    fn build(&self, app: &mut App) {
        ensure_asset_access_registry(app.world_mut());
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
                let path = PathBuf::from(first);
                path.is_dir().then_some(path)
            });
            match pybevy_dir {
                Some(dir) => ApiIndex::build(&dir),
                None => {
                    warn!("[Control] Could not find pybevy package directory for API stubs");
                    ApiIndex::build(Path::new(""))
                }
            }
        } else {
            ApiIndex::build(Path::new(""))
        };

        let control_config = ControlConfig {
            port: config.port,
            host: config.host.clone(),
            screenshot: config.screenshot,
            manipulation: config.manipulation,
            execute_python: config.execute_python,
            api_discovery: config.api_discovery,
            token: config.token.clone(),
        };
        build_control_plugin(app, control_config, api_index, Box::new(Pyo3ControlRuntime));
    }

    fn finish(&self, app: &mut App) {
        install_render_frame_readiness(app);
    }
}

/// Sets `WinitSettings::unfocused_mode` to `Reactive` with a short wait so that
/// MCP/control requests (screenshots, turnarounds, timelines) still run when the
/// window is not focused, without spinning at uncapped FPS.
#[cfg(feature = "bevy_winit")]
fn configure_mcp_update_mode(settings: Option<ResMut<WinitSettings>>) {
    let Some(mut settings) = settings else {
        return;
    };
    // Reactive wakes on input/window/device events AND after the wait duration,
    // so MCP requests that trigger redraws are handled promptly while idle
    // frames are capped to ~60fps instead of spinning continuously.
    let target = UpdateMode::reactive(Duration::from_millis(16));
    if matches!(settings.unfocused_mode, UpdateMode::Reactive { .. }) {
        return;
    }
    settings.unfocused_mode = target;
    info!("[Control] Set unfocused_mode to Reactive(16ms) for MCP responsiveness");
}

#[cfg(test)]
mod tests {
    use bevy::ecs::name::Name;
    #[cfg(feature = "bevy_winit")]
    use bevy::{
        ecs::{system::RunSystemOnce, world::World},
        winit::{UpdateMode, WinitSettings},
    };
    use serde_json::json;
    use tokio::sync::oneshot;

    use super::*;
    use crate::{
        engine::{
            channel::ControlRequest,
            runtime::fake::{Crate, Enemy, FakeRuntime},
        },
        protocol::ControlOperation,
    };

    #[test]
    fn installed_runtime_serves_requests_in_first() {
        let mut app = App::new();
        let config = ControlConfig {
            port: 0,
            host: "127.0.0.1".into(),
            screenshot: true,
            manipulation: true,
            execute_python: true,
            api_discovery: true,
            token: None,
        };
        let (sender, _) = install_control_runtime(&mut app, config, Box::new(FakeRuntime));
        let goblin = app
            .world_mut()
            .spawn((Name::new("Goblin"), Crate, Enemy))
            .id();

        let (response_tx, mut response_rx) = oneshot::channel();
        sender
            .tx
            .send(ControlRequest {
                operation: ControlOperation::GetRegistry,
                response_tx,
            })
            .unwrap();
        app.world_mut().run_schedule(First);

        let result = response_rx.try_recv().unwrap().unwrap();
        assert_eq!(result["component_bridge_names"], json!(["Crate"]));
        assert_eq!(result["resource_names"], json!(["Clock", "Score"]));
        assert_eq!(result["entity_samples"][0]["id"], goblin.to_bits());
        assert_eq!(
            result["entity_samples"][0]["detected_custom_components"],
            json!(["Enemy"])
        );
    }

    #[cfg(feature = "bevy_winit")]
    #[test]
    fn upgrades_continuous_to_reactive() {
        let mut world = World::new();
        world.insert_resource(WinitSettings {
            unfocused_mode: UpdateMode::Continuous,
            ..Default::default()
        });
        world.run_system_once(configure_mcp_update_mode).unwrap();
        let settings = world.resource::<WinitSettings>();
        assert!(matches!(
            settings.unfocused_mode,
            UpdateMode::Reactive { .. }
        ));
    }

    #[cfg(feature = "bevy_winit")]
    #[test]
    fn preserves_already_reactive() {
        let mut world = World::new();
        world.insert_resource(WinitSettings {
            unfocused_mode: UpdateMode::reactive(Duration::from_secs(1)),
            ..Default::default()
        });
        world.run_system_once(configure_mcp_update_mode).unwrap();
        let settings = world.resource::<WinitSettings>();
        // Should keep the existing reactive mode, not overwrite it
        assert!(matches!(
            settings.unfocused_mode,
            UpdateMode::Reactive { .. }
        ));
    }

    #[cfg(feature = "bevy_winit")]
    #[test]
    fn no_op_without_winit_settings() {
        let mut world = World::new();
        world.run_system_once(configure_mcp_update_mode).unwrap();
    }
}
