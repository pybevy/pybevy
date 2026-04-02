pub mod accumulated_mouse;
pub mod axis;
pub mod button_input;
pub mod button_state;
pub mod gamepad;
pub mod gamepad_axis;
pub mod gamepad_button;
pub mod gamepad_event;
pub mod gamepad_events;
pub mod gamepad_input;
pub mod gamepad_rumble_intensity;
pub mod gamepad_rumble_request;
pub mod gamepad_settings;
pub mod gesture_events;
pub mod key_code;
pub mod keyboard_events;
pub mod keyboard_input;
pub mod keyboard_input_ext;
pub mod mouse_button;
pub mod mouse_events;
pub mod mouse_input;
pub mod mouse_scroll_unit;
pub mod plugin;
pub mod touch_input;
pub mod touch_phase;
pub mod touches;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        axis::PyAxis, button_input::PyButtonInput, gamepad::PyGamepad, gamepad_axis::PyGamepadAxis,
        gamepad_button::PyGamepadButton, gamepad_settings::PyGamepadSettings, key_code::PyKeyCode,
        mouse_button::PyMouseButton, plugin::PyInputPlugin, touch_input::PyTouchInput,
        touches::PyTouches,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "input")?;
    m.add_class::<plugin::PyInputPlugin>()?;
    m.add_class::<axis::PyAxis>()?;
    m.add_class::<button_input::PyButtonInput>()?;
    m.add_class::<button_state::PyButtonState>()?;
    m.add_class::<gamepad::PyGamepad>()?;
    m.add_class::<gamepad_axis::PyGamepadAxis>()?;
    m.add_class::<gamepad_button::PyGamepadButton>()?;
    m.add_class::<gamepad_input::PyGamepadInput>()?;
    m.add_class::<gamepad_rumble_intensity::PyGamepadRumbleIntensity>()?;
    m.add_class::<gamepad_rumble_request::PyGamepadRumbleRequest>()?;
    m.add_class::<gamepad_settings::PyGamepadSettings>()?;
    m.add_class::<gamepad_settings::PyButtonSettings>()?;
    m.add_class::<gamepad_settings::PyAxisSettings>()?;
    m.add_class::<gamepad_settings::PyButtonAxisSettings>()?;
    m.add_class::<key_code::PyKeyCode>()?;
    m.add_class::<mouse_button::PyMouseButton>()?;
    m.add_class::<mouse_events::PyMouseButtonInput>()?;
    m.add_class::<mouse_input::PyMouseInput>()?;
    m.add_class::<mouse_events::PyMouseMotion>()?;
    m.add_class::<mouse_scroll_unit::PyMouseScrollUnit>()?;
    m.add_class::<mouse_events::PyMouseWheel>()?;
    m.add_class::<touch_input::PyTouchInput>()?;
    m.add_class::<touch_phase::PyTouchPhase>()?;

    m.add_class::<gamepad_events::PyGamepadButtonChanged>()?;
    m.add_class::<gamepad_events::PyGamepadAxisChanged>()?;
    m.add_class::<gamepad_events::PyGamepadConnection>()?;
    m.add_class::<gamepad_events::PyGamepadButtonStateChanged>()?;
    m.add_class::<gesture_events::PyPinchGesture>()?;
    m.add_class::<gesture_events::PyRotationGesture>()?;
    m.add_class::<gesture_events::PyDoubleTapGesture>()?;
    m.add_class::<gesture_events::PyPanGesture>()?;
    m.add_class::<keyboard_events::PyKeyboardFocusLost>()?;
    m.add_class::<accumulated_mouse::PyAccumulatedMouseMotion>()?;
    m.add_class::<accumulated_mouse::PyAccumulatedMouseScroll>()?;
    m.add_class::<gamepad_event::PyGamepadEvent>()?;
    m.add_class::<keyboard_input::PyKeyboardInput>()?;
    m.add_class::<touches::PyTouch>()?;
    m.add_class::<touches::PyTouches>()?;
    parent.add_submodule(&m)
}
