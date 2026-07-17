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
pub mod present_mode;
pub mod primary_monitor;
pub mod primary_window;
pub mod request_redraw;
pub mod resize_constraints;
pub mod screen_edge;
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
pub mod window_ref;
pub mod window_resized;
pub mod window_resolution;
pub mod window_theme;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        cursor_events::{PyCursorEntered, PyCursorLeft},
        cursor_moved::PyCursorMoved,
        file_drag_and_drop::PyFileDragAndDrop,
        ime::PyIme,
        monitor::PyMonitor,
        monitor_selection::PyMonitorSelection,
        primary_monitor::PyPrimaryMonitor,
        primary_window::PyPrimaryWindow,
        resize_constraints::PyWindowResizeConstraints,
        video_mode_selection::PyVideoModeSelection,
        window::PyWindow,
        window_plugin::PyWindowPlugin,
        window_position::PyWindowPosition,
        window_ref::{PyNormalizedWindowRef, PyWindowRef},
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "window")?;

    m.add_class::<window_plugin::PyWindowPlugin>()?;
    m.add_class::<app_lifecycle::PyAppLifecycle>()?;
    m.add_class::<composite_alpha_mode::PyCompositeAlphaMode>()?;
    m.add_class::<cursor::PyCursorGrabMode>()?;
    m.add_class::<enabled_buttons::PyEnabledButtons>()?;
    m.add_class::<exit_condition::PyExitCondition>()?;
    m.add_class::<monitor_selection::PyMonitorSelection>()?;
    m.add_class::<present_mode::PyPresentMode>()?;
    m.add_class::<screen_edge::PyScreenEdge>()?;
    m.add_class::<cursor::PySystemCursorIcon>()?;
    m.add_class::<video_mode::PyVideoMode>()?;
    m.add_class::<video_mode_selection::PyVideoModeSelection>()?;
    m.add_class::<window_level::PyWindowLevel>()?;
    m.add_class::<window_mode::PyWindowMode>()?;
    m.add_class::<window_position::PyWindowPosition>()?;
    m.add_class::<resize_constraints::PyWindowResizeConstraints>()?;
    m.add_class::<window_resolution::PyWindowResolution>()?;
    m.add_class::<window_ref::PyWindowRef>()?;
    m.add_class::<window_ref::PyNormalizedWindowRef>()?;
    m.add_class::<window_theme::PyWindowTheme>()?;

    m.add_class::<cursor_icon::PyCursorIcon>()?;
    m.add_class::<cursor_options::PyCursorOptions>()?;
    m.add_class::<monitor::PyMonitor>()?;
    m.add_class::<primary_monitor::PyPrimaryMonitor>()?;
    m.add_class::<primary_window::PyPrimaryWindow>()?;
    m.add_class::<window::PyWindow>()?;

    m.add_class::<cursor_events::PyCursorEntered>()?;
    m.add_class::<cursor_events::PyCursorLeft>()?;
    m.add_class::<cursor_moved::PyCursorMoved>()?;
    m.add_class::<file_drag_and_drop::PyFileDragAndDrop>()?;
    m.add_class::<ime::PyIme>()?;
    m.add_class::<request_redraw::PyRequestRedraw>()?;
    m.add_class::<window_close_requested::PyWindowCloseRequested>()?;
    m.add_class::<window_event::PyWindowEvent>()?;
    m.add_class::<window_focused::PyWindowFocused>()?;
    m.add_class::<window_resized::PyWindowResized>()?;

    parent.add_submodule(&m)
}
