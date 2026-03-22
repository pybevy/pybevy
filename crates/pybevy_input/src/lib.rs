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

use std::any::TypeId;

pub use accumulated_mouse::{PyAccumulatedMouseMotion, PyAccumulatedMouseScroll};
pub use axis::PyAxis;
use bevy::{
    ecs::{component::ComponentId, world::World},
    input::gamepad::{Gamepad, GamepadSettings},
};
pub use button_input::PyButtonInput;
pub use button_state::PyButtonState;
pub use gamepad::PyGamepad;
pub use gamepad_axis::PyGamepadAxis;
pub use gamepad_button::PyGamepadButton;
pub use gamepad_event::PyGamepadEvent;
pub use gamepad_events::{
    PyGamepadAxisChanged, PyGamepadButtonChanged, PyGamepadButtonStateChanged, PyGamepadConnection,
};
pub use gamepad_input::PyGamepadInput;
pub use gamepad_rumble_intensity::PyGamepadRumbleIntensity;
pub use gamepad_rumble_request::PyGamepadRumbleRequest;
pub use gamepad_settings::{
    PyAxisSettings, PyButtonAxisSettings, PyButtonSettings, PyGamepadSettings,
};
pub use gesture_events::{PyDoubleTapGesture, PyPanGesture, PyPinchGesture, PyRotationGesture};
pub use key_code::PyKeyCode;
pub use keyboard_events::PyKeyboardFocusLost;
pub use keyboard_input::PyKeyboardInput;
pub use keyboard_input_ext::PyKeyboardInputExt;
pub use mouse_button::PyMouseButton;
pub use mouse_events::{PyMouseButtonInput, PyMouseMotion, PyMouseWheel};
pub use mouse_input::PyMouseInput;
pub use mouse_scroll_unit::PyMouseScrollUnit;
pub use plugin::PyInputPlugin;
use pybevy_core::{
    ValidityFlagWithMode,
    plugin::plugin_registry,
    registry::{ResourceBridge, global_registry},
};
use pybevy_macros::{component_bridge, plugin_bridge, resource_bridge};
use pyo3::{PyTypeInfo, ffi::PyTypeObject, prelude::*, types::PyType};
pub use touch_input::PyTouchInput;
pub use touch_phase::PyTouchPhase;
pub use touches::{PyTouch, PyTouches};

// Generate component bridges
component_bridge!(Gamepad, PyGamepad, no_insert);
component_bridge!(GamepadSettings, PyGamepadSettings, no_insert);

// Generate plugin bridges via macro
plugin_bridge!(PyInputPlugin, bevy::input::InputPlugin);

// Generate resource bridges
resource_bridge!(
    bevy::input::touch::Touches,
    PyTouches,
    no_mut,
    default_insert
);
resource_bridge!(
    bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>,
    PyButtonInput,
    "ButtonInput",
    no_mut,
    default_insert
);
resource_bridge!(
    bevy::input::ButtonInput<bevy::input::mouse::MouseButton>,
    PyMouseInput,
    "MouseInput",
    no_mut,
    default_insert
);
resource_bridge!(
    bevy::input::Axis<bevy::input::gamepad::GamepadAxis>,
    PyAxis,
    "Axis",
    no_mut,
    no_insert
);

// Manual bridge for AccumulatedMouseMotion (value-based, no ResourceStorage)
pub struct AccumulatedMouseMotionBridge;

impl ResourceBridge for AccumulatedMouseMotionBridge {
    fn bevy_type_id(&self) -> TypeId {
        TypeId::of::<bevy::input::mouse::AccumulatedMouseMotion>()
    }

    fn py_type_ptr(&self) -> *const PyTypeObject {
        Python::attach(|py| {
            PyAccumulatedMouseMotion::type_object(py).as_type_ptr() as *const PyTypeObject
        })
    }

    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType> {
        PyAccumulatedMouseMotion::type_object(py)
    }

    fn name(&self) -> &'static str {
        "AccumulatedMouseMotion"
    }

    fn get(
        &self,
        world: &World,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>> {
        let resource = world
            .get_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    "AccumulatedMouseMotion resource not found in world",
                )
            })?;
        let py_resource = Py::new(py, PyAccumulatedMouseMotion::from_bevy(resource))?;
        Ok(py_resource.into_any())
    }

    fn get_mut(
        &self,
        world: &mut World,
        _validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>> {
        let resource = world
            .get_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    "AccumulatedMouseMotion resource not found in world",
                )
            })?;
        let py_resource = Py::new(py, PyAccumulatedMouseMotion::from_bevy(resource))?;
        Ok(py_resource.into_any())
    }

    fn insert(&self, world: &mut World, resource: &Bound<PyAny>) -> PyResult<()> {
        let py_resource = resource.extract::<PyRef<PyAccumulatedMouseMotion>>()?;
        let motion = bevy::input::mouse::AccumulatedMouseMotion {
            delta: py_resource.delta.get(),
        };
        world.insert_resource(motion);
        Ok(())
    }

    fn remove(&self, world: &mut World) {
        world.remove_resource::<bevy::input::mouse::AccumulatedMouseMotion>();
    }

    fn contains_in_world(&self, world: &World) -> bool {
        world.contains_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
    }

    fn resource_id(&self, world: &World) -> Option<ComponentId> {
        world
            .components()
            .resource_id::<bevy::input::mouse::AccumulatedMouseMotion>()
    }
}
pub fn register_input_bridges() {
    global_registry::register_component_bridge(GamepadBridge);
    global_registry::register_component_bridge(GamepadSettingsBridge);

    // Message bridges - touch
    global_registry::register_message_bridge(touch_input::TouchInputBridge);

    // Message bridges - mouse
    global_registry::register_message_bridge(mouse_events::MouseButtonInputBridge);
    global_registry::register_message_bridge(mouse_events::MouseMotionBridge);
    global_registry::register_message_bridge(mouse_events::MouseWheelBridge);

    // Message bridges - gamepad
    global_registry::register_message_bridge(gamepad_events::GamepadButtonChangedBridge);
    global_registry::register_message_bridge(gamepad_events::GamepadAxisChangedBridge);
    global_registry::register_message_bridge(gamepad_events::GamepadConnectionBridge);
    global_registry::register_message_bridge(gamepad_events::GamepadButtonStateChangedBridge);

    // Message bridges - gesture
    global_registry::register_message_bridge(gesture_events::PinchGestureBridge);
    global_registry::register_message_bridge(gesture_events::RotationGestureBridge);
    global_registry::register_message_bridge(gesture_events::DoubleTapGestureBridge);
    global_registry::register_message_bridge(gesture_events::PanGestureBridge);

    // Message bridges - keyboard
    global_registry::register_message_bridge(keyboard_events::KeyboardFocusLostBridge);

    // Resource bridges
    global_registry::register_resource_bridge(AccumulatedMouseMotionBridge);
    global_registry::register_resource_bridge(TouchesBridge);
    global_registry::register_resource_bridge(ButtonInputBridge);
    global_registry::register_resource_bridge(MouseInputBridge);
    global_registry::register_resource_bridge(AxisBridge);

    // Plugins
    plugin_registry::register_plugin_bridge(InputPluginBridge);
}
pub fn add_input_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_input_bridges();

    // Plugins
    m.add_class::<PyInputPlugin>()?;

    m.add_class::<PyAxis>()?;
    m.add_class::<PyButtonInput>()?;
    m.add_class::<PyButtonState>()?;
    m.add_class::<PyGamepad>()?;
    m.add_class::<PyGamepadAxis>()?;
    m.add_class::<PyGamepadButton>()?;
    m.add_class::<PyGamepadInput>()?;
    m.add_class::<PyGamepadRumbleIntensity>()?;
    m.add_class::<PyGamepadRumbleRequest>()?;
    m.add_class::<PyGamepadSettings>()?;
    m.add_class::<PyButtonSettings>()?;
    m.add_class::<PyAxisSettings>()?;
    m.add_class::<PyButtonAxisSettings>()?;
    m.add_class::<PyKeyCode>()?;
    m.add_class::<PyMouseButton>()?;
    m.add_class::<PyMouseButtonInput>()?;
    m.add_class::<PyMouseInput>()?;
    m.add_class::<PyMouseMotion>()?;
    m.add_class::<PyMouseScrollUnit>()?;
    m.add_class::<PyMouseWheel>()?;
    m.add_class::<PyTouchInput>()?;
    m.add_class::<PyTouchPhase>()?;

    // Gamepad events
    m.add_class::<PyGamepadButtonChanged>()?;
    m.add_class::<PyGamepadAxisChanged>()?;
    m.add_class::<PyGamepadConnection>()?;
    m.add_class::<PyGamepadButtonStateChanged>()?;

    // Gesture events
    m.add_class::<PyPinchGesture>()?;
    m.add_class::<PyRotationGesture>()?;
    m.add_class::<PyDoubleTapGesture>()?;
    m.add_class::<PyPanGesture>()?;

    // Keyboard events
    m.add_class::<PyKeyboardFocusLost>()?;

    // Accumulated mouse resources
    m.add_class::<PyAccumulatedMouseMotion>()?;
    m.add_class::<PyAccumulatedMouseScroll>()?;

    // GamepadEvent
    m.add_class::<PyGamepadEvent>()?;

    // KeyboardInput
    m.add_class::<PyKeyboardInput>()?;

    // Touches
    m.add_class::<PyTouch>()?;
    m.add_class::<PyTouches>()?;

    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "input")?;
    add_input_classes(&m)?;
    parent.add_submodule(&m)
}
