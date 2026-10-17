use bevy::{
    input::{
        gestures::{DoubleTapGesture, PanGesture, PinchGesture, RotationGesture},
        keyboard::{KeyboardFocusLost, KeyboardInput},
        mouse::{MouseButtonInput, MouseMotion, MouseWheel},
        touch::TouchInput,
    },
    window::{
        AppLifecycle, CursorEntered, CursorLeft, CursorMoved, FileDragAndDrop, Ime, RequestRedraw,
        WindowBackendScaleFactorChanged, WindowCloseRequested, WindowCreated, WindowDestroyed,
        WindowEvent, WindowFocused, WindowMoved, WindowOccluded, WindowResized,
        WindowScaleFactorChanged, WindowThemeChanged,
    },
};
use pybevy_core::message_payload;
use pybevy_input::{
    gesture_events::{PyDoubleTapGesture, PyPanGesture, PyPinchGesture, PyRotationGesture},
    keyboard_events::PyKeyboardFocusLost,
    keyboard_input::{PyKeyboardInput, materialize_keyboard_input},
    mouse_events::{PyMouseButtonInput, PyMouseMotion, PyMouseWheel},
    touch_input::PyTouchInput,
};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{
    app_lifecycle::PyAppLifecycle,
    cursor_events::{PyCursorEntered, PyCursorLeft},
    cursor_moved::PyCursorMoved,
    file_drag_and_drop::{PyFileDragAndDrop, materialize_file_drag_and_drop},
    ime::{PyIme, materialize_ime},
    request_redraw::PyRequestRedraw,
    window_close_requested::PyWindowCloseRequested,
    window_events::{
        PyWindowBackendScaleFactorChanged, PyWindowCreated, PyWindowDestroyed, PyWindowMoved,
        PyWindowOccluded, PyWindowScaleFactorChanged, PyWindowThemeChanged,
    },
    window_focused::PyWindowFocused,
    window_resized::PyWindowResized,
};

message_payload!(CursorEnteredPayload, CursorEntered, PyCursorEntered);
message_payload!(CursorLeftPayload, CursorLeft, PyCursorLeft);
message_payload!(CursorMovedPayload, CursorMoved, PyCursorMoved);
message_payload!(
    FileDragAndDropPayload,
    FileDragAndDrop,
    PyFileDragAndDrop,
    materialize = materialize_file_drag_and_drop
);
message_payload!(ImePayload, Ime, PyIme, materialize = materialize_ime);
message_payload!(RequestRedrawPayload, RequestRedraw, PyRequestRedraw);
message_payload!(
    WindowBackendScaleFactorChangedPayload,
    WindowBackendScaleFactorChanged,
    PyWindowBackendScaleFactorChanged
);
message_payload!(
    WindowCloseRequestedPayload,
    WindowCloseRequested,
    PyWindowCloseRequested
);
message_payload!(WindowCreatedPayload, WindowCreated, PyWindowCreated);
message_payload!(WindowDestroyedPayload, WindowDestroyed, PyWindowDestroyed);
message_payload!(WindowFocusedPayload, WindowFocused, PyWindowFocused);
message_payload!(WindowMovedPayload, WindowMoved, PyWindowMoved);
message_payload!(WindowOccludedPayload, WindowOccluded, PyWindowOccluded);
message_payload!(WindowResizedPayload, WindowResized, PyWindowResized);
message_payload!(
    WindowScaleFactorChangedPayload,
    WindowScaleFactorChanged,
    PyWindowScaleFactorChanged
);
message_payload!(
    WindowThemeChangedPayload,
    WindowThemeChanged,
    PyWindowThemeChanged
);
message_payload!(
    MouseButtonInputPayload,
    MouseButtonInput,
    PyMouseButtonInput
);
message_payload!(MouseMotionPayload, MouseMotion, PyMouseMotion);
message_payload!(MouseWheelPayload, MouseWheel, PyMouseWheel);
message_payload!(PinchGesturePayload, PinchGesture, PyPinchGesture);
message_payload!(RotationGesturePayload, RotationGesture, PyRotationGesture);
message_payload!(
    DoubleTapGesturePayload,
    DoubleTapGesture,
    PyDoubleTapGesture
);
message_payload!(PanGesturePayload, PanGesture, PyPanGesture);
message_payload!(TouchInputPayload, TouchInput, PyTouchInput);
message_payload!(
    KeyboardInputPayload,
    KeyboardInput,
    PyKeyboardInput,
    materialize = materialize_keyboard_input
);
message_payload!(
    KeyboardFocusLostPayload,
    KeyboardFocusLost,
    PyKeyboardFocusLost
);

#[pyenum(WindowEvent, message, writable)]
#[pyclass(name = "WindowEvent", module = "pybevy.window")]
pub enum PyWindowEvent {
    #[py_bevy(tuple)]
    AppLifecycle {
        #[py_type(PyAppLifecycle)]
        value: AppLifecycle,
    },
    #[py_bevy(tuple)]
    CursorEntered {
        #[py_type(CursorEnteredPayload)]
        value: CursorEntered,
    },
    #[py_bevy(tuple)]
    CursorLeft {
        #[py_type(CursorLeftPayload)]
        value: CursorLeft,
    },
    #[py_bevy(tuple)]
    CursorMoved {
        #[py_type(CursorMovedPayload)]
        value: CursorMoved,
    },
    #[py_bevy(tuple)]
    FileDragAndDrop {
        #[py_type(FileDragAndDropPayload)]
        value: FileDragAndDrop,
    },
    #[py_bevy(tuple)]
    Ime {
        #[py_type(ImePayload)]
        value: Ime,
    },
    #[py_bevy(tuple)]
    RequestRedraw {
        #[py_type(RequestRedrawPayload)]
        value: RequestRedraw,
    },
    #[py_bevy(tuple)]
    WindowBackendScaleFactorChanged {
        #[py_type(WindowBackendScaleFactorChangedPayload)]
        value: WindowBackendScaleFactorChanged,
    },
    #[py_bevy(tuple)]
    WindowCloseRequested {
        #[py_type(WindowCloseRequestedPayload)]
        value: WindowCloseRequested,
    },
    #[py_bevy(tuple)]
    WindowCreated {
        #[py_type(WindowCreatedPayload)]
        value: WindowCreated,
    },
    #[py_bevy(tuple)]
    WindowDestroyed {
        #[py_type(WindowDestroyedPayload)]
        value: WindowDestroyed,
    },
    #[py_bevy(tuple)]
    WindowFocused {
        #[py_type(WindowFocusedPayload)]
        value: WindowFocused,
    },
    #[py_bevy(tuple)]
    WindowMoved {
        #[py_type(WindowMovedPayload)]
        value: WindowMoved,
    },
    #[py_bevy(tuple)]
    WindowOccluded {
        #[py_type(WindowOccludedPayload)]
        value: WindowOccluded,
    },
    #[py_bevy(tuple)]
    WindowResized {
        #[py_type(WindowResizedPayload)]
        value: WindowResized,
    },
    #[py_bevy(tuple)]
    WindowScaleFactorChanged {
        #[py_type(WindowScaleFactorChangedPayload)]
        value: WindowScaleFactorChanged,
    },
    #[py_bevy(tuple)]
    WindowThemeChanged {
        #[py_type(WindowThemeChangedPayload)]
        value: WindowThemeChanged,
    },
    #[py_bevy(tuple)]
    MouseButtonInput {
        #[py_type(MouseButtonInputPayload)]
        value: MouseButtonInput,
    },
    #[py_bevy(tuple)]
    MouseMotion {
        #[py_type(MouseMotionPayload)]
        value: MouseMotion,
    },
    #[py_bevy(tuple)]
    MouseWheel {
        #[py_type(MouseWheelPayload)]
        value: MouseWheel,
    },
    #[py_bevy(tuple)]
    PinchGesture {
        #[py_type(PinchGesturePayload)]
        value: PinchGesture,
    },
    #[py_bevy(tuple)]
    RotationGesture {
        #[py_type(RotationGesturePayload)]
        value: RotationGesture,
    },
    #[py_bevy(tuple)]
    DoubleTapGesture {
        #[py_type(DoubleTapGesturePayload)]
        value: DoubleTapGesture,
    },
    #[py_bevy(tuple)]
    PanGesture {
        #[py_type(PanGesturePayload)]
        value: PanGesture,
    },
    #[py_bevy(tuple)]
    TouchInput {
        #[py_type(TouchInputPayload)]
        value: TouchInput,
    },
    #[py_bevy(tuple)]
    KeyboardInput {
        #[py_type(KeyboardInputPayload)]
        value: KeyboardInput,
    },
    #[py_bevy(tuple)]
    KeyboardFocusLost {
        #[py_type(KeyboardFocusLostPayload)]
        value: KeyboardFocusLost,
    },
}

impl From<&PyWindowEvent> for WindowEvent {
    fn from(value: &PyWindowEvent) -> Self {
        value.clone().into()
    }
}
