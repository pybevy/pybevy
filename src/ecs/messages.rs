use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bevy::{
    asset::AssetEvent,
    ecs::{
        component::ComponentId,
        message::{Message, MessageCursor, Messages},
        resource::Resource,
        world::World,
    },
    image::Image,
    input::{
        ButtonInput,
        keyboard::{KeyCode, KeyboardInput},
    },
    mesh::Mesh,
    window::WindowEvent,
};
use pybevy_core::registry::global_registry;
use pyo3::{
    IntoPyObjectExt, exceptions::PyTypeError, ffi::PyTypeObject, prelude::*, types::PyType,
};

/// Persistent cursor storage shared between DynamicSystem and PyMessages.
/// The Arc+Mutex allows frozen pyclass fields to hold mutable state.
pub(crate) type CursorStorage = Arc<Mutex<Option<Box<dyn Any + Send + Sync>>>>;

use super::message::{PyMessage, PyMessageId};
use crate::ecs::{resource::PyResource, world::PyWorld};

// Macro to generate CustomMessage types
macro_rules! define_custom_messages {
    ($($n:literal),+ $(,)?) => {
        paste::paste! {
            $(
                #[derive(Message)]
                pub(crate) struct [<CustomMessage $n>](pub(crate) Py<PyAny>);
            )+
        }
    };
}

// Macro to generate match arms for message registration using add_message
// Maps message_num (0-19) to CustomMessage types (1-20)
// Uses app.add_message() which adds BOTH the Messages resource AND the update system
macro_rules! add_message_arms {
    ($message_num:expr, $app:expr, $($idx:literal => $type_num:literal),+ $(,)?) => {
        paste::paste! {
            match $message_num {
                $(
                    $idx => {
                        $app.add_message::<[<CustomMessage $type_num>]>();
                        // Get the ComponentId for the Messages resource
                        $app.world().resource_id::<Messages<[<CustomMessage $type_num>]>>().unwrap()
                    }
                )+
                _ => unreachable!(),
            }
        }
    };
}

// Macro to generate match arms for reading messages with persistent cursor support
macro_rules! read_message_arms {
    ($message_num:expr, $world:expr, $py:expr, $result:expr, $cursor_storage:expr, $($idx:literal => $type_num:literal),+ $(,)?) => {
        paste::paste! {
            match $message_num {
                $(
                    $idx => {
                        if let Some(messages) = $world.get_resource::<Messages<[<CustomMessage $type_num>]>>() {
                            let mut cursor = $cursor_storage
                                .as_ref()
                                .and_then(|s| s.downcast_ref::<MessageCursor<[<CustomMessage $type_num>]>>())
                                .cloned()
                                .unwrap_or_else(|| messages.get_cursor());
                            for msg in cursor.read(messages) {
                                $result.push(msg.0.clone_ref($py));
                            }
                            *$cursor_storage = Some(Box::new(cursor));
                        }
                    }
                )+
                _ => return Err(PyTypeError::new_err("Invalid message number")),
            }
        }
    };
}

// Macro to generate match arms for with_messages
macro_rules! with_messages_arms {
    ($message_num:expr, $world:expr, $f:expr, $($idx:literal => $type_num:literal),+ $(,)?) => {
        paste::paste! {
            match $message_num {
                $(
                    $idx => {
                        let mut messages = $world.get_resource_or_insert_with(|| Messages::<[<CustomMessage $type_num>]>::default());
                        Ok($f(&mut Wrap(&mut *messages)))
                    }
                )+
                _ => Err(PyTypeError::new_err("Invalid message number")),
            }
        }
    };
}

/// Python-facing wrapper for Bevy message resources (events).
///
/// Provides read and write access to a specific message type's resource,
/// used by `MessageReader` and `MessageWriter` system parameters.
#[pyclass(name = "Messages", extends = PyResource, frozen)]
pub struct PyMessages {
    pub(crate) message_type: MessageType,
    pub(crate) world: PyWorld,
    /// Persistent cursor for MessageReader iteration.
    /// When Some, iter_to_python uses the stored cursor to avoid re-reading messages.
    /// When None, creates a fresh cursor each time (legacy behavior).
    pub(crate) cursor_storage: Option<CursorStorage>,
}

impl PyMessages {
    /// Generic helper to iterate messages and convert to Python objects.
    /// Uses persistent cursor when cursor_storage is available.
    fn iter_messages<BevyMsg, F>(
        &self,
        py: Python,
        world: &mut bevy::ecs::world::World,
        cursor_state: &mut Option<Box<dyn Any + Send + Sync>>,
        convert: F,
    ) -> PyResult<Vec<Py<PyAny>>>
    where
        BevyMsg: Message + 'static,
        F: Fn(&BevyMsg, Python) -> PyResult<Py<PyAny>>,
    {
        let messages = world.get_resource::<Messages<BevyMsg>>();
        if let Some(messages_res) = messages {
            let mut cursor = cursor_state
                .as_ref()
                .and_then(|s| s.downcast_ref::<MessageCursor<BevyMsg>>())
                .cloned()
                .unwrap_or_else(|| messages_res.get_cursor());
            let mut result = Vec::new();
            for msg in cursor.read(messages_res) {
                let py_msg = convert(msg, py)?;
                result.push(py_msg);
            }
            *cursor_state = Some(Box::new(cursor));
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    /// Lock cursor storage (if available) and return a mutable reference to the inner state.
    /// Returns a default None state when no cursor storage is configured.
    fn lock_cursor(&self) -> Option<std::sync::MutexGuard<'_, Option<Box<dyn Any + Send + Sync>>>> {
        self.cursor_storage.as_ref().map(|cs| cs.lock().unwrap())
    }

    pub(crate) fn iter_to_python(&self, py: Python) -> PyResult<Vec<Py<PyAny>>> {
        use pybevy_input::{PyKeyboardInput, PyKeyboardInputExt};

        let world = self.world.world_mut()?;
        let mut guard = self.lock_cursor();
        let cursor_state = match guard.as_mut() {
            Some(g) => &mut **g,
            None => {
                // No persistent cursor — use a temporary that's discarded
                &mut None
            }
        };

        match &self.message_type {
            MessageType::KeyboardInput => {
                // Special handling: needs ButtonInput<KeyCode> for modifier detection
                let messages = world.get_resource::<Messages<KeyboardInput>>();
                let keyboard = world.get_resource::<ButtonInput<KeyCode>>();

                if let (Some(messages_res), Some(keyboard)) = (messages, keyboard) {
                    let mut cursor = cursor_state
                        .as_ref()
                        .and_then(|s| s.downcast_ref::<MessageCursor<KeyboardInput>>())
                        .cloned()
                        .unwrap_or_else(|| messages_res.get_cursor());
                    let mut result = Vec::new();
                    for msg in cursor.read(messages_res) {
                        if let Some(py_msg_tuple) = PyKeyboardInput::from_bevy(msg, keyboard) {
                            let py_msg = Py::new(py, py_msg_tuple)?;
                            result.push(py_msg.into_any());
                        }
                    }
                    *cursor_state = Some(Box::new(cursor));
                    Ok(result)
                } else {
                    Ok(Vec::new())
                }
            }
            MessageType::GamepadRumbleRequest => {
                // Write-only message type, no reading support
                Ok(Vec::new())
            }
            MessageType::WindowEvent => {
                use pybevy_window::PyWindowEvent;
                self.iter_messages::<WindowEvent, _>(py, world, cursor_state, |msg, py| {
                    Ok(Py::new(py, PyWindowEvent::from_bevy(py, msg)?)?.into_any())
                })
            }
            MessageType::SceneInstanceReady => {
                use pybevy_scene::{PySceneInstanceReady, SceneInstanceReadyMessage};
                self.iter_messages::<SceneInstanceReadyMessage, _>(
                    py,
                    world,
                    cursor_state,
                    |msg, py| Ok(Py::new(py, PySceneInstanceReady::from(&msg.0))?.into_any()),
                )
            }
            MessageType::AssetEventImage => {
                use crate::assets::asset_event::PyAssetEvent;
                self.iter_messages::<AssetEvent<Image>, _>(py, world, cursor_state, |event, py| {
                    Ok(Py::new(py, (PyAssetEvent::from_bevy(event), PyMessage))?.into_any())
                })
            }
            MessageType::AssetEventMesh => {
                use crate::assets::asset_event::PyAssetEvent;
                self.iter_messages::<AssetEvent<Mesh>, _>(py, world, cursor_state, |event, py| {
                    Ok(Py::new(py, (PyAssetEvent::from_bevy(event), PyMessage))?.into_any())
                })
            }
            MessageType::Custom(py_type) => {
                // Look up the message number from registry
                let type_ptr = py_type.bind(py).as_type_ptr();
                let message_num = {
                    let Some(registry) = world.get_resource::<MessageRegistry>() else {
                        return Err(PyTypeError::new_err(
                            "MessageRegistry not initialized. Call app.add_message(T) to register message types.",
                        ));
                    };
                    registry
                        .get(type_ptr)
                        .ok_or_else(|| PyTypeError::new_err("Message type not registered"))?
                        .message_num
                };

                // Read from the appropriate CustomMessageN resource
                let mut result = Vec::new();
                read_message_arms!(message_num, world, py, result, cursor_state,
                    0=>1, 1=>2, 2=>3, 3=>4, 4=>5, 5=>6, 6=>7, 7=>8, 8=>9, 9=>10,
                    10=>11, 11=>12, 12=>13, 13=>14, 14=>15, 15=>16, 16=>17, 17=>18, 18=>19, 19=>20
                );
                Ok(result)
            }
            MessageType::Dynamic(type_ptr) => {
                // Use global registry for dynamic dispatch (most message types use this)
                let bridge =
                    global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                        PyTypeError::new_err("Message type not registered in global registry")
                    })?;
                bridge.iter_to_python_with_cursor(py, world, cursor_state)
            }
        }
    }

    fn with_messages<F, R>(&self, f: F) -> PyResult<R>
    where
        F: FnOnce(&mut dyn ErasedMessages) -> R,
    {
        let world = self.world.world_mut()?;

        match &self.message_type {
            MessageType::KeyboardInput => {
                let mut messages =
                    world.get_resource_or_insert_with(Messages::<KeyboardInput>::default);
                Ok(f(&mut Wrap(&mut *messages)))
            }
            MessageType::GamepadRumbleRequest => {
                // Write-only message type
                Err(PyTypeError::new_err(
                    "GamepadRumbleRequest events are write-only (generated by gamepad system)",
                ))
            }
            MessageType::WindowEvent => {
                let mut messages =
                    world.get_resource_or_insert_with(Messages::<WindowEvent>::default);
                Ok(f(&mut Wrap(&mut *messages)))
            }
            MessageType::SceneInstanceReady => {
                use pybevy_scene::SceneInstanceReadyMessage;
                let mut messages = world.get_resource_or_insert_with(|| {
                    Messages::<SceneInstanceReadyMessage>::default()
                });
                Ok(f(&mut Wrap(&mut *messages)))
            }
            MessageType::AssetEventImage => {
                let mut messages =
                    world.get_resource_or_insert_with(Messages::<AssetEvent<Image>>::default);
                Ok(f(&mut Wrap(&mut *messages)))
            }
            MessageType::AssetEventMesh => {
                let mut messages =
                    world.get_resource_or_insert_with(Messages::<AssetEvent<Mesh>>::default);
                Ok(f(&mut Wrap(&mut *messages)))
            }
            MessageType::Custom(py_type) => {
                // Look up the message number from registry
                let message_num = Python::attach(|py| {
                    let type_ptr = py_type.bind(py).as_type_ptr();
                    let Some(registry) = world.get_resource::<MessageRegistry>() else {
                        return Err(PyTypeError::new_err(
                            "MessageRegistry not initialized. Call app.add_message(T) to register message types.",
                        ));
                    };
                    registry
                        .get(type_ptr)
                        .ok_or_else(|| PyTypeError::new_err("Message type not registered"))
                        .map(|reg| reg.message_num)
                })?;

                // Access the appropriate CustomMessageN resource
                with_messages_arms!(message_num, world, f,
                    0=>1, 1=>2, 2=>3, 3=>4, 4=>5, 5=>6, 6=>7, 7=>8, 8=>9, 9=>10,
                    10=>11, 11=>12, 12=>13, 13=>14, 14=>15, 15=>16, 16=>17, 17=>18, 18=>19, 19=>20
                )
            }
            MessageType::Dynamic(_) => {
                // Dynamic types are handled directly in the public methods before calling with_messages
                unreachable!("Dynamic message types should be handled before with_messages()")
            }
        }
    }
}

#[allow(dead_code)] // implemented by Wrap<T>, invoked via dyn dispatch
trait ErasedMessages {
    fn send(&mut self, _message: PyMessage) -> PyMessageId;
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn iter_messages(&self, py: Python) -> PyResult<Vec<Py<PyAny>>>;
}

struct Wrap<'a, T: Message>(pub &'a mut Messages<T>);

impl<'a, T: Message> ErasedMessages for Wrap<'a, T> {
    fn send(&mut self, _message: PyMessage) -> PyMessageId {
        // Not reachable: PyMessages.send() raises NotImplementedError before calling this
        unreachable!("ErasedMessages::send should not be called directly")
    }

    fn clear(&mut self) {
        self.0.clear();
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn iter_messages(&self, _py: Python) -> PyResult<Vec<Py<PyAny>>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "iter_messages not implemented for this message type",
        ))
    }
}

#[pymethods]
impl PyMessages {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let ty = PyMessageType::from_message_type(key.cast::<PyType>()?)?;
        PyMessageTypeParam(ty).into_py_any(py)
    }

    pub fn send(&self, _py: Python, _message: Bound<'_, PyMessage>) -> PyResult<PyMessageId> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Messages.send() is not implemented. Use MessageWriter[T] as a system parameter to send messages.",
        ))
    }

    pub fn clear(&self) -> PyResult<()> {
        if let MessageType::Dynamic(type_ptr) = &self.message_type {
            let bridge =
                global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                    PyTypeError::new_err("Message type not registered in global registry")
                })?;
            let world = self.world.world_mut()?;
            return bridge.clear(world);
        }
        self.with_messages(|messages| messages.clear())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        if let MessageType::Dynamic(type_ptr) = &self.message_type {
            let bridge =
                global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                    PyTypeError::new_err("Message type not registered in global registry")
                })?;
            let world = self.world.world_mut()?;
            return bridge.is_empty(world);
        }
        self.with_messages(|messages| messages.is_empty())
    }

    pub fn len(&self) -> PyResult<usize> {
        if let MessageType::Dynamic(type_ptr) = &self.message_type {
            let bridge =
                global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                    PyTypeError::new_err("Message type not registered in global registry")
                })?;
            let world = self.world.world_mut()?;
            return bridge.len(world);
        }
        self.with_messages(|messages| messages.len())
    }
}

#[derive(Debug)]
pub enum MessageType {
    // Special handling required (no bridge)
    KeyboardInput,
    GamepadRumbleRequest,
    WindowEvent,
    SceneInstanceReady,
    AssetEventImage,
    #[allow(dead_code)] // variant used by message bridge system
    AssetEventMesh,
    // Python custom messages
    Custom(Py<PyType>),
    /// Dynamic message type registered via global message bridge registry.
    /// All event types with message_bridge! use this variant.
    Dynamic(*const pyo3::ffi::PyTypeObject),
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for MessageType {}
unsafe impl Sync for MessageType {}

impl PartialEq for MessageType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MessageType::KeyboardInput, MessageType::KeyboardInput) => true,
            (MessageType::GamepadRumbleRequest, MessageType::GamepadRumbleRequest) => true,
            (MessageType::WindowEvent, MessageType::WindowEvent) => true,
            (MessageType::SceneInstanceReady, MessageType::SceneInstanceReady) => true,
            (MessageType::AssetEventImage, MessageType::AssetEventImage) => true,
            (MessageType::AssetEventMesh, MessageType::AssetEventMesh) => true,
            (MessageType::Custom(a), MessageType::Custom(b)) => a.is(b),
            (MessageType::Dynamic(a), MessageType::Dynamic(b)) => std::ptr::eq(*a, *b),
            _ => false,
        }
    }
}

impl Clone for MessageType {
    fn clone(&self) -> Self {
        match self {
            MessageType::KeyboardInput => MessageType::KeyboardInput,
            MessageType::GamepadRumbleRequest => MessageType::GamepadRumbleRequest,
            MessageType::WindowEvent => MessageType::WindowEvent,
            MessageType::SceneInstanceReady => MessageType::SceneInstanceReady,
            MessageType::AssetEventImage => MessageType::AssetEventImage,
            MessageType::AssetEventMesh => MessageType::AssetEventMesh,
            MessageType::Custom(ty) => Python::attach(|py| MessageType::Custom(ty.clone_ref(py))),
            MessageType::Dynamic(ptr) => MessageType::Dynamic(*ptr),
        }
    }
}

impl MessageType {
    /// Get the ComponentId for the Messages<T> resource this message type maps to.
    ///
    /// Used by DynamicSystem::initialize() to register read/write access for messages.
    pub(crate) fn resource_id(&self, world: &World) -> Option<ComponentId> {
        match self {
            MessageType::KeyboardInput => {
                world.components().resource_id::<Messages<KeyboardInput>>()
            }
            MessageType::GamepadRumbleRequest => {
                // PyBevy-only type with no actual Messages resource
                None
            }
            MessageType::WindowEvent => world.components().resource_id::<Messages<WindowEvent>>(),
            MessageType::SceneInstanceReady => world
                .components()
                .resource_id::<Messages<pybevy_scene::SceneInstanceReadyMessage>>(),
            MessageType::AssetEventImage => world
                .components()
                .resource_id::<Messages<AssetEvent<Image>>>(),
            MessageType::AssetEventMesh => world
                .components()
                .resource_id::<Messages<AssetEvent<Mesh>>>(),
            MessageType::Custom(py_type) => {
                let type_ptr = Python::attach(|py| py_type.bind(py).as_type_ptr());
                world
                    .get_resource::<MessageRegistry>()
                    .and_then(|registry| registry.get(type_ptr))
                    .map(|registered| registered.component_id)
            }
            MessageType::Dynamic(type_ptr) => {
                global_registry::get_message_bridge_by_py_type(*type_ptr)
                    .and_then(|bridge| bridge.resource_id(world))
            }
        }
    }
}

#[pyclass(name = "MessageType", eq, frozen)]
#[derive(Debug, PartialEq, Clone)]
pub struct PyMessageType(pub(crate) MessageType);

impl PyMessageType {
    pub(crate) fn from_message_type(message: &Bound<'_, PyType>) -> PyResult<Self> {
        use pybevy_input::{PyGamepadRumbleRequest, PyKeyboardInput};
        use pybevy_scene::PySceneInstanceReady;
        use pybevy_window::PyWindowEvent;
        use pyo3::{PyTypeInfo, types::PyTypeMethods};

        let py = message.py();
        let type_ptr = message.as_type_ptr();

        // Check global message bridge registry first (dynamic dispatch)
        // Most message types use this path via message_bridge! macro
        if pybevy_core::registry::global_registry::contains_message_py_type(type_ptr) {
            return Ok(PyMessageType(MessageType::Dynamic(type_ptr)));
        }

        // Special handling types that don't use message_bridge! (need extra resources/special logic)
        if message.is(<PyKeyboardInput as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::KeyboardInput));
        }

        if message.is(<PyGamepadRumbleRequest as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::GamepadRumbleRequest));
        }

        if message.is(<PyWindowEvent as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::WindowEvent));
        }

        if message.is(<PySceneInstanceReady as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::SceneInstanceReady));
        }

        // Check for AssetEvent types
        use crate::assets::asset_event::PyAssetEvent;

        // For now, we identify AssetEvent by checking if it's the AssetEvent class
        // In the future, we might need to distinguish Image vs Mesh events differently
        if message.is(<PyAssetEvent as PyTypeInfo>::type_object(py)) {
            // Default to Image events for now - this will need refinement
            // when we expose separate AssetEvent[Image] and AssetEvent[Mesh] types
            return Ok(PyMessageType(MessageType::AssetEventImage));
        }

        // Check for generic PyMessage subclass (custom messages)
        if message.is_subclass_of::<PyMessage>().unwrap_or(false) {
            Ok(PyMessageType(MessageType::Custom(
                message.as_unbound().clone_ref(py),
            )))
        } else {
            Err(PyTypeError::new_err(
                "Message type must be a subclass of `Message`",
            ))
        }
    }
}

#[derive(Resource, Default)]
pub struct CustomMessageState {
    registered_count: usize,
}

pub(crate) struct RegisteredMessage {
    // The component ID of the resource
    pub component_id: ComponentId,
    pub message_num: usize,
    pub pytype: Py<PyType>,
    /// Generation when this registration was created or last aliased.
    /// Used by prune_old_generations() to remove stale type pointer entries.
    pub generation: u32,
}

// Generate CustomMessage1 through CustomMessage20 using macro
define_custom_messages!(
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20
);

/// Registry mapping Python message types to their ComponentIds
#[derive(Default, Resource)]
pub(crate) struct MessageRegistry {
    registry: HashMap<*const PyTypeObject, RegisteredMessage>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for MessageRegistry {}
unsafe impl Sync for MessageRegistry {}

impl MessageRegistry {
    /// Get the RegisteredMessage for a Python message type
    pub(crate) fn get(&self, type_ptr: *const PyTypeObject) -> Option<&RegisteredMessage> {
        self.registry.get(&type_ptr)
    }

    /// Add a new type pointer as an alias for an existing registration found by name.
    /// Used during hot reload: Python classes are recreated with new PyTypeObject pointers,
    /// but MCP tool dispatchers still hold old class references. Keeping both old and new
    /// pointers ensures both can resolve to the same message slot.
    /// Returns true if an alias was added.
    pub(crate) fn alias_by_name(
        &mut self,
        new_type_ptr: *const PyTypeObject,
        name: &str,
        new_pytype: Py<PyType>,
        generation: u32,
    ) -> bool {
        // Find the existing registration by class name
        let existing = Python::attach(|py| {
            self.registry.iter().find_map(|(_ptr, reg)| {
                let type_name = reg
                    .pytype
                    .bind(py)
                    .getattr("__name__")
                    .and_then(|n| n.extract::<String>())
                    .unwrap_or_default();
                if type_name == name {
                    Some(reg.message_num)
                } else {
                    None
                }
            })
        });

        if let Some(message_num) = existing {
            // Add the new pointer as an alias pointing to the same slot
            self.registry.insert(
                new_type_ptr,
                RegisteredMessage {
                    component_id: ComponentId::new(0), // Not used for lookup
                    message_num,
                    pytype: new_pytype,
                    generation,
                },
            );
            true
        } else {
            false
        }
    }

    /// Remove stale type pointer entries from generations older than `min_generation`.
    /// Keeps at least one entry per message_num (the newest one) to ensure messages
    /// remain functional. Only removes duplicate pointer aliases from old reloads.
    pub(crate) fn prune_old_generations(&mut self, min_generation: u32) {
        use std::collections::HashMap as StdHashMap;

        // Find the newest generation for each message_num
        let mut newest_per_slot: StdHashMap<usize, u32> = StdHashMap::new();
        for reg in self.registry.values() {
            let entry = newest_per_slot.entry(reg.message_num).or_insert(0);
            if reg.generation > *entry {
                *entry = reg.generation;
            }
        }

        // Remove entries that are both old AND not the newest for their slot
        self.registry.retain(|_ptr, reg| {
            if reg.generation >= min_generation {
                return true; // Recent enough, keep
            }
            // Old entry: keep only if it's the newest for its slot
            newest_per_slot
                .get(&reg.message_num)
                .is_none_or(|&newest| reg.generation >= newest)
        });
    }

    pub(crate) fn register_message(
        py: Python,
        message: &Bound<'_, PyType>,
        app: &mut bevy::app::App,
    ) -> PyResult<()> {
        if !message.is_subclass_of::<PyMessage>()? {
            return Err(PyTypeError::new_err("Expected a subclass of `Message`"));
        }

        let type_ptr = message.as_type_ptr();
        let world = app.world_mut();

        // Ensure registry exists
        if !world.contains_resource::<MessageRegistry>() {
            world.init_resource::<MessageRegistry>();
        }
        if !world.contains_resource::<CustomMessageState>() {
            world.init_resource::<CustomMessageState>();
        }

        // Check if already registered
        {
            let registry = world.resource::<MessageRegistry>();
            if registry.get(type_ptr).is_some() {
                return Ok(()); // Already registered
            }
        }

        // Get the next message number
        let message_num = {
            let state = world.resource::<CustomMessageState>();
            state.registered_count
        };

        // Support up to 20 custom message types
        if message_num >= 20 {
            return Err(PyTypeError::new_err(
                "Maximum of 20 custom message types supported",
            ));
        }

        // Register the appropriate Messages<CustomMessageN> resource using add_message
        // This ensures both the resource AND the update system are added for double-buffering
        let component_id = add_message_arms!(message_num, app,
            0=>1, 1=>2, 2=>3, 3=>4, 4=>5, 5=>6, 6=>7, 7=>8, 8=>9, 9=>10,
            10=>11, 11=>12, 12=>13, 13=>14, 14=>15, 15=>16, 16=>17, 17=>18, 18=>19, 19=>20
        );

        // Store in registry
        {
            let world = app.world_mut();
            let mut registry = world.resource_mut::<MessageRegistry>();
            registry.registry.insert(
                type_ptr,
                RegisteredMessage {
                    component_id,
                    message_num,
                    pytype: message.as_unbound().clone_ref(py),
                    generation: 0, // Initial registration, generation 0
                },
            );
        }

        // Increment count
        {
            let world = app.world_mut();
            let mut state = world.resource_mut::<CustomMessageState>();
            state.registered_count += 1;
        }

        Ok(())
    }
}

#[pyclass(name = "MessageTypeParam", frozen)]
#[derive(Debug, PartialEq)]
pub struct PyMessageTypeParam(pub(crate) PyMessageType);
