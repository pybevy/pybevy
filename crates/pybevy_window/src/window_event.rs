use bevy::window::{FileDragAndDrop, Ime, WindowEvent};
use pybevy_core::{PyEntity, PyMessage};
use pybevy_input::{
    button_state::PyButtonState, keyboard_input::PyKeyboardInput, mouse_button::PyMouseButton,
    mouse_scroll_unit::PyMouseScrollUnit, touch_phase::PyTouchPhase,
};
use pybevy_math::{ivec2::PyIVec2, vec2::PyVec2};
use pyo3::{Borrowed, IntoPyObjectExt, prelude::*};

use crate::{
    app_lifecycle::PyAppLifecycle,
    file_drag_and_drop::{FileDragAndDropValue, PyFileDragAndDrop, materialize_file_drag_and_drop},
    ime::{PyIme, materialize_ime},
    window_theme::PyWindowTheme,
};

/// Carries bevy's own value so `WindowEvent` keeps the `Clone` and `PartialEq`
/// its bevy counterpart has, while Python sees the real variant class.
#[derive(Debug, Clone, PartialEq)]
pub struct ImePayload(pub Ime);

impl<'py> IntoPyObject<'py> for ImePayload {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(materialize_ime(py, &self.0)?.into_bound(py))
    }
}

impl FromPyObject<'_, '_> for ImePayload {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let base = obj.extract::<PyRef<'_, PyIme>>()?;
        Ok(ImePayload(Ime::from(base.clone())))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileDragAndDropPayload(pub FileDragAndDrop);

impl<'py> IntoPyObject<'py> for FileDragAndDropPayload {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(materialize_file_drag_and_drop(py, &self.0)?.into_bound(py))
    }
}

impl FromPyObject<'_, '_> for FileDragAndDropPayload {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let base = obj.extract::<PyRef<'_, PyFileDragAndDrop>>()?;
        Ok(FileDragAndDropPayload(FileDragAndDrop::from(
            FileDragAndDropValue::from(base.clone()),
        )))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardInputPayload(pub PyKeyboardInput);

impl<'py> IntoPyObject<'py> for KeyboardInputPayload {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(Py::new(py, (self.0, PyMessage))?.into_bound(py).into_any())
    }
}

impl FromPyObject<'_, '_> for KeyboardInputPayload {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        Ok(Self(obj.extract::<PyRef<'_, PyKeyboardInput>>()?.clone()))
    }
}

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
    #[pyo3(constructor = (value,))]
    FileDragAndDrop {
        value: FileDragAndDropPayload,
    },
    #[pyo3(constructor = (value,))]
    Ime {
        value: ImePayload,
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
    #[pyo3(constructor = (value,))]
    KeyboardInput {
        value: KeyboardInputPayload,
    },
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
            WindowEvent::FileDragAndDrop(e) => PyWindowEvent::FileDragAndDrop {
                value: FileDragAndDropPayload(e.clone()),
            },
            WindowEvent::Ime(e) => PyWindowEvent::Ime {
                value: ImePayload(e.clone()),
            },
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
            WindowEvent::KeyboardInput(event) => PyWindowEvent::KeyboardInput {
                value: KeyboardInputPayload(PyKeyboardInput::from_bevy_event(event)?),
            },
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

// Render every field through Python's exact repr implementation.
fn field_repr<'py, T: IntoPyObject<'py>>(py: Python<'py>, value: T) -> PyResult<String> {
    Ok(value.into_bound_py_any(py)?.repr()?.to_string())
}

#[pymethods]
impl PyWindowEvent {
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(match self {
            PyWindowEvent::AppLifecycle { lifecycle } => format!(
                "WindowEvent.AppLifecycle(lifecycle={})",
                field_repr(py, *lifecycle)?
            ),
            PyWindowEvent::CursorEntered { window } => format!(
                "WindowEvent.CursorEntered(window={})",
                field_repr(py, *window)?
            ),
            PyWindowEvent::CursorLeft { window } => {
                format!(
                    "WindowEvent.CursorLeft(window={})",
                    field_repr(py, *window)?
                )
            }
            PyWindowEvent::CursorMoved {
                position,
                window,
                delta,
            } => format!(
                "WindowEvent.CursorMoved(position={}, window={}, delta={})",
                field_repr(py, position.clone())?,
                field_repr(py, *window)?,
                field_repr(py, delta.clone())?
            ),
            PyWindowEvent::FileDragAndDrop { value } => format!(
                "WindowEvent.FileDragAndDrop(value={})",
                field_repr(py, value.clone())?
            ),
            PyWindowEvent::Ime { value } => {
                format!("WindowEvent.Ime(value={})", field_repr(py, value.clone())?)
            }
            PyWindowEvent::RequestRedraw {} => "WindowEvent.RequestRedraw()".to_string(),
            PyWindowEvent::WindowCloseRequested { window } => format!(
                "WindowEvent.WindowCloseRequested(window={})",
                field_repr(py, *window)?
            ),
            PyWindowEvent::WindowFocused { focused, window } => format!(
                "WindowEvent.WindowFocused(focused={}, window={})",
                field_repr(py, *focused)?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::WindowResized {
                width,
                height,
                window,
            } => format!(
                "WindowEvent.WindowResized(width={}, height={}, window={})",
                field_repr(py, *width)?,
                field_repr(py, *height)?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::MouseButtonInput {
                button,
                state,
                window,
            } => format!(
                "WindowEvent.MouseButtonInput(button={}, state={}, window={})",
                field_repr(py, *button)?,
                field_repr(py, *state)?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::MouseMotion { delta } => format!(
                "WindowEvent.MouseMotion(delta={})",
                field_repr(py, delta.clone())?
            ),
            PyWindowEvent::MouseWheel { unit, x, y, window } => format!(
                "WindowEvent.MouseWheel(unit={}, x={}, y={}, window={})",
                field_repr(py, *unit)?,
                field_repr(py, *x)?,
                field_repr(py, *y)?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::PinchGesture { value } => format!(
                "WindowEvent.PinchGesture(value={})",
                field_repr(py, *value)?
            ),
            PyWindowEvent::RotationGesture { value } => format!(
                "WindowEvent.RotationGesture(value={})",
                field_repr(py, *value)?
            ),
            PyWindowEvent::DoubleTapGesture {} => "WindowEvent.DoubleTapGesture()".to_string(),
            PyWindowEvent::PanGesture { x, y } => format!(
                "WindowEvent.PanGesture(x={}, y={})",
                field_repr(py, *x)?,
                field_repr(py, *y)?
            ),
            PyWindowEvent::TouchInput {
                phase,
                position,
                id,
                window,
                force,
            } => format!(
                "WindowEvent.TouchInput(phase={}, position={}, id={}, window={}, force={})",
                field_repr(py, *phase)?,
                field_repr(py, position.clone())?,
                field_repr(py, *id)?,
                field_repr(py, *window)?,
                field_repr(py, *force)?
            ),
            PyWindowEvent::KeyboardFocusLost {} => "WindowEvent.KeyboardFocusLost()".to_string(),
            PyWindowEvent::WindowCreated { window } => format!(
                "WindowEvent.WindowCreated(window={})",
                field_repr(py, *window)?
            ),
            PyWindowEvent::WindowDestroyed { window } => format!(
                "WindowEvent.WindowDestroyed(window={})",
                field_repr(py, *window)?
            ),
            PyWindowEvent::WindowMoved { position, window } => format!(
                "WindowEvent.WindowMoved(position={}, window={})",
                field_repr(py, position.clone())?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::WindowOccluded { occluded, window } => format!(
                "WindowEvent.WindowOccluded(occluded={}, window={})",
                field_repr(py, *occluded)?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::WindowScaleFactorChanged {
                scale_factor,
                window,
            } => format!(
                "WindowEvent.WindowScaleFactorChanged(scale_factor={}, window={})",
                field_repr(py, *scale_factor)?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::WindowBackendScaleFactorChanged {
                scale_factor,
                window,
            } => format!(
                "WindowEvent.WindowBackendScaleFactorChanged(scale_factor={}, window={})",
                field_repr(py, *scale_factor)?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::WindowThemeChanged { theme, window } => format!(
                "WindowEvent.WindowThemeChanged(theme={}, window={})",
                field_repr(py, *theme)?,
                field_repr(py, *window)?
            ),
            PyWindowEvent::KeyboardInput { value } => format!(
                "WindowEvent.KeyboardInput(value={})",
                field_repr(py, value.clone())?
            ),
        })
    }
}
