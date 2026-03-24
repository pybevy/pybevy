pub mod app_lifecycle;
pub mod composite_alpha_mode;
pub mod cursor;
pub mod cursor_events;
pub mod cursor_icon;
pub mod cursor_moved;
pub mod cursor_options;
pub mod enabled_buttons;
pub mod exit_condition;
pub mod file_drag_and_drop;
pub mod ime;
pub mod monitor;
pub mod monitor_selection;
pub mod plugin;
pub mod present_mode;
pub mod primary_monitor;
pub mod primary_window;
pub mod request_redraw;
pub mod resize_constraints;
pub mod screen_edge;
pub mod update_mode;
pub mod video_mode;
pub mod video_mode_selection;
pub mod window;
pub mod window_close_requested;
pub mod window_event;
pub mod window_focused;
pub mod window_level;
pub mod window_mode;
pub mod window_plugin;
pub mod window_position;
pub mod window_resized;
pub mod window_resolution;
pub mod window_theme;
pub mod winit_settings;

pub use app_lifecycle::PyAppLifecycle;
use bevy::{
    app::{App, Plugin},
    window::{CursorIcon, CursorOptions, Monitor, PrimaryMonitor, PrimaryWindow, Window},
};
pub use composite_alpha_mode::PyCompositeAlphaMode;
pub use cursor::{PyCursorGrabMode, PySystemCursorIcon};
pub use cursor_events::{PyCursorEntered, PyCursorLeft};
pub use cursor_icon::PyCursorIcon;
pub use cursor_moved::PyCursorMoved;
pub use cursor_options::PyCursorOptions;
pub use enabled_buttons::PyEnabledButtons;
pub use exit_condition::PyExitCondition;
pub use file_drag_and_drop::PyFileDragAndDrop;
pub use ime::PyIme;
pub use monitor::PyMonitor;
pub use monitor_selection::PyMonitorSelection;
pub use plugin::PyWinitPlugin;
pub use present_mode::PyPresentMode;
pub use primary_monitor::PyPrimaryMonitor;
pub use primary_window::PyPrimaryWindow;
use pybevy_core::{DynamicComponentRegistry, plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{component_bridge, plugin_bridge};
use pyo3::prelude::*;
pub use request_redraw::PyRequestRedraw;
pub use resize_constraints::PyWindowResizeConstraints;
pub use screen_edge::PyScreenEdge;
pub use update_mode::PyUpdateMode;
pub use video_mode::PyVideoMode;
pub use video_mode_selection::PyVideoModeSelection;
pub use window::{DEFAULT_APP_TITLE, PyWindow};
pub use window_close_requested::PyWindowCloseRequested;
pub use window_event::PyWindowEvent;
pub use window_focused::PyWindowFocused;
pub use window_level::PyWindowLevel;
pub use window_mode::PyWindowMode;
pub use window_plugin::PyWindowPlugin;
pub use window_position::PyWindowPosition;
pub use window_resized::PyWindowResized;
pub use window_resolution::PyWindowResolution;
pub use window_theme::PyWindowTheme;
pub use winit_settings::PyWinitSettings;

// Generate component bridges via macro
component_bridge!(CursorIcon, PyCursorIcon);
component_bridge!(CursorOptions, PyCursorOptions);
component_bridge!(Monitor, PyMonitor, no_insert);
component_bridge!(PrimaryMonitor, PyPrimaryMonitor);
component_bridge!(PrimaryWindow, PyPrimaryWindow);
component_bridge!(
    Window,
    PyWindow,
    view_fields = [decorations, resizable, transparent]
);

// Generate plugin bridges via macro
plugin_bridge!(PyWinitPlugin, bevy::winit::WinitPlugin, |py_plugin, app| {
    let config: pyo3::PyRef<'_, PyWinitPlugin> = py_plugin.extract()?;
    if let Some(ref settings) = config.settings {
        app.insert_resource(bevy::winit::WinitSettings::from(settings.clone()));
    }
    app.add_plugins(bevy::winit::WinitPlugin::default());
    Ok(())
});
plugin_bridge!(
    PyWindowPlugin,
    bevy::window::WindowPlugin,
    |py_plugin, app| {
        let config: pyo3::PyRef<'_, PyWindowPlugin> = py_plugin.extract()?;
        app.add_plugins(bevy::window::WindowPlugin::try_from(&*config)?);
        Ok(())
    }
);

pub struct PyBevyWindowPlugin;

impl Plugin for PyBevyWindowPlugin {
    fn build(&self, app: &mut App) {
        // Register with global registry for type lookup without World access
        global_registry::register_component_bridge(CursorIconBridge);
        global_registry::register_component_bridge(CursorOptionsBridge);
        global_registry::register_component_bridge(MonitorBridge);
        global_registry::register_component_bridge(PrimaryMonitorBridge);
        global_registry::register_component_bridge(PrimaryWindowBridge);
        global_registry::register_component_bridge(WindowBridge);

        // Register component bridges with the Bevy resource
        if let Some(mut registry) = app
            .world_mut()
            .get_resource_mut::<DynamicComponentRegistry>()
        {
            registry.register(CursorIconBridge);
            registry.register(CursorOptionsBridge);
            registry.register(MonitorBridge);
            registry.register(PrimaryMonitorBridge);
            registry.register(PrimaryWindowBridge);
            registry.register(WindowBridge);
        }
    }
}

pub fn register_window_bridges() {
    global_registry::register_component_bridge(CursorIconBridge);
    global_registry::register_component_bridge(CursorOptionsBridge);
    global_registry::register_component_bridge(MonitorBridge);
    global_registry::register_component_bridge(PrimaryMonitorBridge);
    global_registry::register_component_bridge(PrimaryWindowBridge);
    global_registry::register_component_bridge(WindowBridge);
    register_window_batch();

    // Message bridges
    global_registry::register_message_bridge(cursor_events::CursorEnteredBridge);
    global_registry::register_message_bridge(cursor_events::CursorLeftBridge);
    global_registry::register_message_bridge(cursor_moved::CursorMovedBridge);
    global_registry::register_message_bridge(file_drag_and_drop::FileDragAndDropBridge);
    global_registry::register_message_bridge(ime::ImeBridge);
    global_registry::register_message_bridge(request_redraw::RequestRedrawBridge);
    global_registry::register_message_bridge(window_close_requested::WindowCloseRequestedBridge);
    global_registry::register_message_bridge(window_focused::WindowFocusedBridge);
    global_registry::register_message_bridge(window_resized::WindowResizedBridge);

    // Plugins
    plugin_registry::register_plugin_bridge(WinitPluginBridge);
    plugin_registry::register_plugin_bridge(WindowPluginBridge);
}

pub fn add_window_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_window_bridges();

    // Plugins
    m.add_class::<PyWinitPlugin>()?;
    m.add_class::<PyWindowPlugin>()?;

    // Enums and simple types
    m.add_class::<PyAppLifecycle>()?;
    m.add_class::<PyCompositeAlphaMode>()?;
    m.add_class::<PyCursorGrabMode>()?;
    m.add_class::<PyEnabledButtons>()?;
    m.add_class::<PyExitCondition>()?;
    m.add_class::<PyMonitorSelection>()?;
    m.add_class::<PyPresentMode>()?;
    m.add_class::<PyScreenEdge>()?;
    m.add_class::<PySystemCursorIcon>()?;
    m.add_class::<PyVideoMode>()?;
    m.add_class::<PyVideoModeSelection>()?;
    m.add_class::<PyWindowLevel>()?;
    m.add_class::<PyWindowMode>()?;
    m.add_class::<PyWindowPosition>()?;
    m.add_class::<PyWindowResizeConstraints>()?;
    m.add_class::<PyWindowResolution>()?;
    m.add_class::<PyWindowTheme>()?;

    // Components
    m.add_class::<PyCursorIcon>()?;
    m.add_class::<PyCursorOptions>()?;
    m.add_class::<PyMonitor>()?;
    m.add_class::<PyPrimaryMonitor>()?;
    m.add_class::<PyPrimaryWindow>()?;
    m.add_class::<PyWindow>()?;

    // Message types
    m.add_class::<PyCursorEntered>()?;
    m.add_class::<PyCursorLeft>()?;
    m.add_class::<PyCursorMoved>()?;
    m.add_class::<PyFileDragAndDrop>()?;
    m.add_class::<PyIme>()?;
    m.add_class::<PyRequestRedraw>()?;
    m.add_class::<PyWindowCloseRequested>()?;
    m.add_class::<PyWindowEvent>()?;
    m.add_class::<PyWindowFocused>()?;
    m.add_class::<PyWindowResized>()?;

    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "window")?;
    add_window_classes(&m)?;
    parent.add_submodule(&m)
}

pub fn add_winit_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "winit")?;
    m.add_class::<PyWinitPlugin>()?;
    m.add_class::<PyUpdateMode>()?;
    m.add_class::<PyWinitSettings>()?;
    parent.add_submodule(&m)
}
