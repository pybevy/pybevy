use bevy::window::WindowEvent;
use pybevy_core::PyEntity;
use pybevy_input::{
    button_state::PyButtonState, mouse_button::PyMouseButton, mouse_scroll_unit::PyMouseScrollUnit,
    touch_phase::PyTouchPhase,
};
use pybevy_math::{ivec2::PyIVec2, vec2::PyVec2};
use pyo3::prelude::*;

use crate::{app_lifecycle::PyAppLifecycle, window_theme::PyWindowTheme};

#[pyclass(
    name = "WindowEvent",
    module = "pybevy.window",
    eq,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyWindowEvent {
    #[pyo3(constructor = (lifecycle,))]
    AppLifecycle {
        lifecycle: PyAppLifecycle,
    },
    #[pyo3(constructor = (window,))]
    CursorEntered {
        window: PyEntity,
    },
    #[pyo3(constructor = (window,))]
    CursorLeft {
        window: PyEntity,
    },
    #[pyo3(constructor = (position, window, delta = None))]
    CursorMoved {
        position: PyVec2,
        window: PyEntity,
        delta: Option<PyVec2>,
    },
    #[pyo3(constructor = (window, path = None))]
    FileDragAndDrop {
        window: PyEntity,
        path: Option<String>,
    },
    #[pyo3(constructor = (window,))]
    Ime {
        window: PyEntity,
    },
    RequestRedraw {},
    #[pyo3(constructor = (window,))]
    WindowCloseRequested {
        window: PyEntity,
    },
    #[pyo3(constructor = (focused, window))]
    WindowFocused {
        focused: bool,
        window: PyEntity,
    },
    #[pyo3(constructor = (width, height, window))]
    WindowResized {
        width: f32,
        height: f32,
        window: PyEntity,
    },
    #[pyo3(constructor = (button, state, window))]
    MouseButtonInput {
        button: PyMouseButton,
        state: PyButtonState,
        window: PyEntity,
    },
    #[pyo3(constructor = (delta,))]
    MouseMotion {
        delta: PyVec2,
    },
    #[pyo3(constructor = (unit, x, y, window))]
    MouseWheel {
        unit: PyMouseScrollUnit,
        x: f32,
        y: f32,
        window: PyEntity,
    },
    #[pyo3(constructor = (value,))]
    PinchGesture {
        value: f32,
    },
    #[pyo3(constructor = (value,))]
    RotationGesture {
        value: f32,
    },
    DoubleTapGesture {},
    #[pyo3(constructor = (x, y))]
    PanGesture {
        x: f32,
        y: f32,
    },
    #[pyo3(constructor = (phase, position, id, window, force = None))]
    TouchInput {
        phase: PyTouchPhase,
        position: PyVec2,
        id: u64,
        window: PyEntity,
        force: Option<f64>,
    },
    KeyboardFocusLost {},
    #[pyo3(constructor = (window,))]
    WindowCreated {
        window: PyEntity,
    },
    #[pyo3(constructor = (window,))]
    WindowDestroyed {
        window: PyEntity,
    },
    #[pyo3(constructor = (position, window))]
    WindowMoved {
        position: PyIVec2,
        window: PyEntity,
    },
    #[pyo3(constructor = (occluded, window))]
    WindowOccluded {
        occluded: bool,
        window: PyEntity,
    },
    #[pyo3(constructor = (scale_factor, window))]
    WindowScaleFactorChanged {
        scale_factor: f64,
        window: PyEntity,
    },
    #[pyo3(constructor = (scale_factor, window))]
    WindowBackendScaleFactorChanged {
        scale_factor: f64,
        window: PyEntity,
    },
    #[pyo3(constructor = (theme, window))]
    WindowThemeChanged {
        theme: PyWindowTheme,
        window: PyEntity,
    },
    /// TODO REVIEW: KeyboardInput not supported - use MessageReader[KeyboardInput] directly
    KeyboardInput {},
}

impl PyWindowEvent {
    pub fn from_bevy(_py: Python, event: &WindowEvent) -> PyResult<Self> {
        Ok(match event {
            WindowEvent::AppLifecycle(e) => PyWindowEvent::AppLifecycle {
                lifecycle: (*e).into(),
            },
            WindowEvent::CursorEntered(e) => PyWindowEvent::CursorEntered {
                window: e.window.into(),
            },
            WindowEvent::CursorLeft(e) => PyWindowEvent::CursorLeft {
                window: e.window.into(),
            },
            WindowEvent::CursorMoved(e) => PyWindowEvent::CursorMoved {
                position: e.position.into(),
                window: e.window.into(),
                delta: e.delta.map(Into::into),
            },
            WindowEvent::FileDragAndDrop(e) => {
                use bevy::window::FileDragAndDrop;
                let (window, path) = match e {
                    FileDragAndDrop::DroppedFile { window, path_buf } => (
                        (*window).into(),
                        Some(path_buf.to_string_lossy().to_string()),
                    ),
                    FileDragAndDrop::HoveredFile { window, path_buf } => (
                        (*window).into(),
                        Some(path_buf.to_string_lossy().to_string()),
                    ),
                    FileDragAndDrop::HoveredFileCanceled { window } => ((*window).into(), None),
                };
                PyWindowEvent::FileDragAndDrop { window, path }
            }
            WindowEvent::Ime(e) => {
                use bevy::window::Ime;
                let window = match e {
                    Ime::Preedit { window, .. } => *window,
                    Ime::Commit { window, .. } => *window,
                    Ime::Enabled { window } => *window,
                    Ime::Disabled { window } => *window,
                };
                PyWindowEvent::Ime {
                    window: window.into(),
                }
            }
            WindowEvent::RequestRedraw(_) => PyWindowEvent::RequestRedraw {},
            WindowEvent::WindowCloseRequested(e) => PyWindowEvent::WindowCloseRequested {
                window: e.window.into(),
            },
            WindowEvent::WindowFocused(e) => PyWindowEvent::WindowFocused {
                focused: e.focused,
                window: e.window.into(),
            },
            WindowEvent::WindowResized(e) => PyWindowEvent::WindowResized {
                width: e.width,
                height: e.height,
                window: e.window.into(),
            },
            WindowEvent::MouseButtonInput(e) => PyWindowEvent::MouseButtonInput {
                button: e.button.into(),
                state: e.state.into(),
                window: e.window.into(),
            },
            WindowEvent::MouseMotion(e) => PyWindowEvent::MouseMotion {
                delta: e.delta.into(),
            },
            WindowEvent::MouseWheel(e) => PyWindowEvent::MouseWheel {
                unit: e.unit.into(),
                x: e.x,
                y: e.y,
                window: e.window.into(),
            },
            WindowEvent::PinchGesture(e) => PyWindowEvent::PinchGesture { value: e.0 },
            WindowEvent::RotationGesture(e) => PyWindowEvent::RotationGesture { value: e.0 },
            WindowEvent::DoubleTapGesture(_) => PyWindowEvent::DoubleTapGesture {},
            WindowEvent::PanGesture(e) => PyWindowEvent::PanGesture { x: e.0.x, y: e.0.y },
            WindowEvent::TouchInput(e) => {
                let force = e.force.map(|f| match f {
                    bevy::input::touch::ForceTouch::Calibrated {
                        force,
                        max_possible_force,
                        ..
                    } => force / max_possible_force,
                    bevy::input::touch::ForceTouch::Normalized(n) => n,
                });
                PyWindowEvent::TouchInput {
                    phase: e.phase.into(),
                    position: e.position.into(),
                    id: e.id,
                    window: e.window.into(),
                    force,
                }
            }
            WindowEvent::KeyboardInput(_) => {
                // KeyboardInput requires ButtonInput<KeyCode> for modifier detection
                // Use MessageReader[KeyboardInput] directly for full keyboard support
                PyWindowEvent::KeyboardInput {}
            }
            WindowEvent::KeyboardFocusLost(_) => PyWindowEvent::KeyboardFocusLost {},
            WindowEvent::WindowCreated(e) => PyWindowEvent::WindowCreated {
                window: e.window.into(),
            },
            WindowEvent::WindowDestroyed(e) => PyWindowEvent::WindowDestroyed {
                window: e.window.into(),
            },
            WindowEvent::WindowMoved(e) => PyWindowEvent::WindowMoved {
                position: e.position.into(),
                window: e.window.into(),
            },
            WindowEvent::WindowOccluded(e) => PyWindowEvent::WindowOccluded {
                occluded: e.occluded,
                window: e.window.into(),
            },
            WindowEvent::WindowScaleFactorChanged(e) => PyWindowEvent::WindowScaleFactorChanged {
                scale_factor: e.scale_factor,
                window: e.window.into(),
            },
            WindowEvent::WindowBackendScaleFactorChanged(e) => {
                PyWindowEvent::WindowBackendScaleFactorChanged {
                    scale_factor: e.scale_factor,
                    window: e.window.into(),
                }
            }
            WindowEvent::WindowThemeChanged(e) => PyWindowEvent::WindowThemeChanged {
                theme: e.theme.into(),
                window: e.window.into(),
            },
        })
    }
}

#[pymethods]
impl PyWindowEvent {
    fn __repr__(&self) -> String {
        format!("{:?}", self)
    }
}
