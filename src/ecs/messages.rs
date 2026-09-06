use std::{
    any::Any,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        component::ComponentId,
        message::{Message, MessageCursor, Messages},
        world::{World, unsafe_world_cell::UnsafeWorldCell},
    },
    input::keyboard::KeyboardInput,
    window::{WindowClosing, WindowCreated, WindowEvent, WindowResized, WindowScaleFactorChanged},
};
use pybevy_core::{
    public_error::{
        ASSET_BRIDGE_NOT_FOUND, ASSET_EVENT_TYPE_REQUIRED, ASSET_LOAD_FAILED_TYPE_REQUIRED,
    },
    registry::global_registry,
};
use pybevy_ecs::shared::message_resources::ensure_message_resource;
use pybevy_world_serialization::WorldInstanceReadyMessage;
use pyo3::{
    IntoPyObjectExt, PyTraverseError, PyVisit,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyTuple, PyType},
};

/// Persistent cursor storage shared between DynamicSystem and PyMessages.
/// The Arc+Mutex allows frozen pyclass fields to hold mutable state.
pub(crate) type CursorStorage = Arc<Mutex<Option<Box<dyn Any + Send + Sync>>>>;

use super::message::{PyMessage, PyMessageId};
use crate::{
    assets::asset_load_failed_event::{
        PyAssetLoadFailedEvent, materialize_asset_load_failed_record,
    },
    ecs::{
        dynamic_system::lock_or_recover, helpers::validity_guard::ValidityFlag,
        resource::PyResource,
    },
};

/// Narrow world access for message wrappers.
///
/// Holds the lifetime-erased [`UnsafeWorldCell`] (fenced by the same `ValidityFlag`
/// as `PyQueryIter`) and derives a momentary `&mut World` per operation. The
/// native message adapters/bridges only reach their `Messages<T>` resource,
/// which `DynamicSystem::initialize` declares. This is the same
/// residual-pointer class as `query_runtime::world_ptr`.
#[derive(Clone)]
pub(crate) struct MessageWorld {
    cell: UnsafeWorldCell<'static>,
    validity: ValidityFlag,
}

// SAFETY: mirrors PyQueryIter's discipline. The cell is only touched while the
// owning system runs on a single thread, fenced by `validity`.
unsafe impl Send for MessageWorld {}
unsafe impl Sync for MessageWorld {}

impl MessageWorld {
    /// # Safety
    /// `cell` must reference the world holding the message buffers and stay valid
    /// for as long as `validity` is active.
    pub(crate) unsafe fn new(cell: UnsafeWorldCell, validity: ValidityFlag) -> Self {
        // SAFETY: layout-preserving lifetime erasure of a Copy pointer type; the
        // cell is only touched while `validity` is active.
        let cell: UnsafeWorldCell<'static> = unsafe { std::mem::transmute(cell) };
        Self { cell, validity }
    }

    /// Momentary `&mut World` for the message macros/bridges (they take `&mut World`
    /// but only touch the declared `Messages<T>` resource).
    ///
    /// SAFETY of the returned reference: `initialize` declares reads for
    /// reader resource ids and writes for writer ids; the executor prevents a
    /// conflicting system running concurrently, so the actual message access is
    /// unique. Same residual-pointer class as `query_runtime::world_ptr`.
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn world_mut(&self) -> PyResult<&mut World> {
        self.validity.check()?;
        // SAFETY: momentary derivation; see method docs.
        Ok(unsafe { self.cell.world_mut() })
    }
}

/// Python-facing wrapper for Bevy message resources (events).
///
/// Provides read and write access to a specific message type's resource,
/// used by `MessageReader` and `MessageWriter` system parameters.
#[pyclass(name = "Messages", module = "pybevy.ecs", extends = PyResource, frozen)]
pub struct PyMessages {
    pub(crate) message_type: MessageType,
    pub(crate) world: MessageWorld,
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
        self.cursor_storage.as_ref().map(|cs| lock_or_recover(cs))
    }

    pub(crate) fn iter_to_python(&self, py: Python) -> PyResult<Vec<Py<PyAny>>> {
        use pybevy_input::{
            keyboard_input::PyKeyboardInput, keyboard_input_ext::PyKeyboardInputExt,
        };

        let world = self.world.world_mut()?;
        let mut guard = self.lock_cursor();
        let cursor_state = match guard.as_mut() {
            Some(g) => &mut **g,
            None => {
                // No persistent cursor: use a temporary that's discarded
                &mut None
            }
        };

        match &self.message_type {
            MessageType::KeyboardInput => {
                self.iter_messages::<KeyboardInput, _>(py, world, cursor_state, |msg, py| {
                    Ok(Py::new(py, PyKeyboardInput::from_bevy(msg)?)?.into_any())
                })
            }
            MessageType::GamepadRumbleRequest => {
                // Write-only message type, no reading support
                Ok(Vec::new())
            }
            MessageType::WindowEvent => {
                use pybevy_window::window_event::PyWindowEvent;
                self.iter_messages::<WindowEvent, _>(py, world, cursor_state, |msg, py| {
                    Ok(PyWindowEvent::from_bevy(py, msg)?
                        .into_pyobject(py)?
                        .into_any()
                        .unbind())
                })
            }
            MessageType::WorldInstanceReady => {
                use pybevy_world_serialization::world_instance_ready::PyWorldInstanceReady;
                self.iter_messages::<WorldInstanceReadyMessage, _>(
                    py,
                    world,
                    cursor_state,
                    |msg, py| {
                        Ok(
                            Py::new(py, (PyWorldInstanceReady::from(&msg.0), PyMessage))?
                                .into_any(),
                        )
                    },
                )
            }
            MessageType::AssetEvent(type_ptr) => {
                use crate::assets::asset_event::materialize_asset_event_record;

                let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
                bridge
                    .read_events(world, cursor_state)
                    .into_iter()
                    .map(|event| materialize_asset_event_record(py, event))
                    .collect()
            }
            MessageType::AssetLoadFailed(type_ptr) => {
                let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
                bridge
                    .read_load_failed_events(world, cursor_state)
                    .into_iter()
                    .map(|event| materialize_asset_load_failed_record(py, event))
                    .collect()
            }
            MessageType::Custom(_) => Err(PyTypeError::new_err(
                "custom messages use the shared Python message store",
            )),
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

        // clear/is_empty/len back these arms; a missing buffer reads as empty
        // (EmptyMessages) rather than inserting a resource, which would be a
        // structural world mutation from a possibly-parallel system.
        match &self.message_type {
            MessageType::KeyboardInput => {
                match world.get_resource_mut::<Messages<KeyboardInput>>() {
                    Some(mut messages) => Ok(f(&mut Wrap(&mut *messages))),
                    None => Ok(f(&mut EmptyMessages)),
                }
            }
            MessageType::GamepadRumbleRequest => {
                // Write-only message type with no readable buffer.
                Err(PyTypeError::new_err(
                    "GamepadRumbleRequest is write-only and not accessible through MessageReader or MessageWriter",
                ))
            }
            MessageType::WindowEvent => match world.get_resource_mut::<Messages<WindowEvent>>() {
                Some(mut messages) => Ok(f(&mut Wrap(&mut *messages))),
                None => Ok(f(&mut EmptyMessages)),
            },
            MessageType::WorldInstanceReady => {
                match world.get_resource_mut::<Messages<WorldInstanceReadyMessage>>() {
                    Some(mut messages) => Ok(f(&mut Wrap(&mut *messages))),
                    None => Ok(f(&mut EmptyMessages)),
                }
            }
            MessageType::AssetEvent(_) => {
                unreachable!("AssetEvent message types are handled directly before with_messages()")
            }
            MessageType::AssetLoadFailed(_) => {
                unreachable!(
                    "AssetLoadFailedEvent message types are handled directly before with_messages()"
                )
            }
            MessageType::Custom(_) => Err(PyTypeError::new_err(
                "custom messages use the shared Python message store",
            )),
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

/// Stand-in for an absent `Messages<T>` resource. Reader-side maintenance
/// (clear/is_empty/len) treats a missing buffer as empty instead of inserting one.
struct EmptyMessages;

impl ErasedMessages for EmptyMessages {
    fn send(&mut self, _message: PyMessage) -> PyMessageId {
        unreachable!("EmptyMessages::send should not be called")
    }

    fn clear(&mut self) {}

    fn is_empty(&self) -> bool {
        true
    }

    fn len(&self) -> usize {
        0
    }

    fn iter_messages(&self, _py: Python) -> PyResult<Vec<Py<PyAny>>> {
        Ok(Vec::new())
    }
}

#[pymethods]
impl PyMessages {
    /// Report a custom message class held by the legacy `Messages[T]` wrapper.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        self.message_type.traverse(&visit)
    }

    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let ty = PyMessageType::from_annotation(key)?;
        PyMessageTypeParam(ty).into_py_any(py)
    }

    pub fn send(&self, _py: Python, _message: Bound<'_, PyMessage>) -> PyResult<PyMessageId> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Messages.send() is not implemented. Use MessageWriter[T] as a system parameter to send messages.",
        ))
    }

    pub fn clear(&self) -> PyResult<()> {
        if let MessageType::AssetEvent(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return {
                let _: () = bridge.clear_events(self.world.world_mut()?);
                Ok(())
            };
        }
        if let MessageType::AssetLoadFailed(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            bridge.clear_load_failed_events(self.world.world_mut()?);
            return Ok(());
        }
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
        if let MessageType::AssetEvent(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return Ok(bridge.events_is_empty(self.world.world_mut()?));
        }
        if let MessageType::AssetLoadFailed(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return Ok(bridge.load_failed_events_is_empty(self.world.world_mut()?));
        }
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
        if let MessageType::AssetEvent(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return Ok(bridge.event_count(self.world.world_mut()?));
        }
        if let MessageType::AssetLoadFailed(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return Ok(bridge.load_failed_event_count(self.world.world_mut()?));
        }
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
    WorldInstanceReady,
    AssetEvent(*const pyo3::ffi::PyTypeObject),
    AssetLoadFailed(*const pyo3::ffi::PyTypeObject),
    // Python custom messages
    Custom(Py<PyType>),
    /// Dynamic message type registered via global message bridge registry.
    /// All event types with #[pymessage] use this variant.
    Dynamic(*const pyo3::ffi::PyTypeObject),
}

impl MessageType {
    /// Report a `Custom` variant's class to the cyclic GC.
    ///
    /// `Dynamic` holds a bare pointer to a module-level wrapper class and the
    /// remaining variants hold nothing, so only `Custom` owns a reference.
    pub(crate) fn traverse(&self, visit: &PyVisit<'_>) -> Result<(), PyTraverseError> {
        match self {
            MessageType::Custom(class) => visit.call(class),
            _ => Ok(()),
        }
    }
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
            (MessageType::WorldInstanceReady, MessageType::WorldInstanceReady) => true,
            (MessageType::AssetEvent(a), MessageType::AssetEvent(b)) => std::ptr::eq(*a, *b),
            (MessageType::AssetLoadFailed(a), MessageType::AssetLoadFailed(b)) => {
                std::ptr::eq(*a, *b)
            }
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
            MessageType::WorldInstanceReady => MessageType::WorldInstanceReady,
            MessageType::AssetEvent(ptr) => MessageType::AssetEvent(*ptr),
            MessageType::AssetLoadFailed(ptr) => MessageType::AssetLoadFailed(*ptr),
            MessageType::Custom(ty) => Python::attach(|py| MessageType::Custom(ty.clone_ref(py))),
            MessageType::Dynamic(ptr) => MessageType::Dynamic(*ptr),
        }
    }
}

impl MessageType {
    /// Stable exact channel identity and a user-facing validation label.
    pub(crate) fn validation_identity(&self) -> (String, String) {
        match self {
            Self::KeyboardInput => (
                "native:KeyboardInput".to_string(),
                "Message<KeyboardInput>".to_string(),
            ),
            Self::GamepadRumbleRequest => (
                "native:GamepadRumbleRequest".to_string(),
                "Message<GamepadRumbleRequest>".to_string(),
            ),
            Self::WindowEvent => (
                "native:WindowEvent".to_string(),
                "Message<WindowEvent>".to_string(),
            ),
            Self::WorldInstanceReady => (
                "native:WorldInstanceReady".to_string(),
                "Message<WorldInstanceReady>".to_string(),
            ),
            Self::AssetEvent(type_ptr) => {
                let name = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .map_or("Asset", |bridge| bridge.name());
                (
                    format!("native:AssetEvent<{name}>"),
                    format!("Message<AssetEvent<{name}>>"),
                )
            }
            Self::AssetLoadFailed(type_ptr) => {
                let name = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .map_or("Asset", |bridge| bridge.name());
                (
                    format!("native:AssetLoadFailedEvent<{name}>"),
                    format!("Message<AssetLoadFailedEvent<{name}>>"),
                )
            }
            Self::Custom(class) => Python::attach(|py| {
                let class = class.bind(py);
                let module = class
                    .getattr("__module__")
                    .and_then(|value| value.extract::<String>())
                    .unwrap_or_else(|_| "<unknown>".to_string());
                let qualname = class
                    .getattr("__qualname__")
                    .and_then(|value| value.extract::<String>())
                    .unwrap_or_else(|_| "<custom>".to_string());
                (
                    format!("custom:{module}.{qualname}"),
                    format!("Message<{module}.{qualname}>"),
                )
            }),
            Self::Dynamic(type_ptr) => {
                let name = global_registry::get_message_bridge_by_py_type(*type_ptr)
                    .map_or("Dynamic", |bridge| bridge.name());
                (format!("dynamic:{type_ptr:p}"), format!("Message<{name}>"))
            }
        }
    }

    /// Register (get-or-create) the ComponentId for a native `Messages<T>` resource.
    ///
    /// `DynamicSystem::initialize` calls this before the first write so native
    /// message access is never silently omitted. Custom Python messages declare
    /// their shared-store and synthetic channel access in `MainResolver` instead.
    pub(crate) fn register_resource_id(&self, world: &mut World) -> Option<ComponentId> {
        match self {
            MessageType::KeyboardInput => {
                Some(world.register_component::<Messages<KeyboardInput>>())
            }
            MessageType::GamepadRumbleRequest => {
                // PyBevy-only type with no `Messages` resource. Its reader yields
                // nothing (iter_to_python returns empty) and its writer returns an
                // error without touching the world (see message.rs / with_messages),
                // so no world state is ever accessed and leaving it undeclared is sound.
                None
            }
            MessageType::WindowEvent => Some(world.register_component::<Messages<WindowEvent>>()),
            MessageType::WorldInstanceReady => {
                Some(world.register_component::<Messages<WorldInstanceReadyMessage>>())
            }
            MessageType::AssetEvent(type_ptr) => {
                global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .map(|bridge| bridge.register_event_resource_id(world))
            }
            MessageType::AssetLoadFailed(type_ptr) => {
                global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .map(|bridge| bridge.register_load_failed_resource_id(world))
            }
            MessageType::Custom(_) => None,
            MessageType::Dynamic(type_ptr) => {
                global_registry::get_message_bridge_by_py_type(*type_ptr)
                    .map(|bridge| bridge.register_resource_id(world))
            }
        }
    }

    /// All resource read ids a MessageReader for this type touches.
    /// `DynamicSystem::initialize` declares every id so the parallel executor
    /// accounts for the full read set.
    pub(crate) fn reader_resource_ids(&self, world: &mut World) -> Vec<ComponentId> {
        let mut ids = Vec::new();
        if let Some(id) = self.register_resource_id(world) {
            ids.push(id);
        }
        ids
    }
}

#[pyclass(
    name = "MessageType",
    module = "pybevy.ecs",
    eq,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, PartialEq, Clone)]
pub struct PyMessageType(pub(crate) MessageType);

impl PyMessageType {
    pub(crate) fn from_annotation(message: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(message_type) = message.extract::<PyRef<'_, PyMessageType>>() {
            return Ok(message_type.clone());
        }
        Self::from_message_type(message.cast::<PyType>()?)
    }

    pub(crate) fn from_message_type(message: &Bound<'_, PyType>) -> PyResult<Self> {
        use pybevy_input::{
            gamepad_rumble_request::PyGamepadRumbleRequest, keyboard_input::PyKeyboardInput,
        };
        use pybevy_window::window_event::PyWindowEvent;
        use pybevy_world_serialization::world_instance_ready::PyWorldInstanceReady;
        use pyo3::{PyTypeInfo, types::PyTypeMethods};

        let py = message.py();
        let type_ptr = message.as_type_ptr();

        // Check global message bridge registry first (dynamic dispatch)
        // Most message types use this path via #[pymessage] attribute
        if pybevy_core::registry::global_registry::contains_message_py_type(type_ptr) {
            return Ok(PyMessageType(MessageType::Dynamic(type_ptr)));
        }

        // Enum-backed message instances use nested variant classes, while the
        // bridge is registered on their common message base class.
        let mro = message.getattr("__mro__")?.cast_into::<PyTuple>()?;
        for base in mro.iter().skip(1) {
            let base = base.cast_into::<PyType>()?;
            let base_ptr = base.as_type_ptr();
            if pybevy_core::registry::global_registry::contains_message_py_type(base_ptr) {
                return Ok(PyMessageType(MessageType::Dynamic(base_ptr)));
            }
        }

        // Special handling types that don't use #[pymessage] (need extra resources/special logic)
        if message.is(<PyKeyboardInput as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::KeyboardInput));
        }

        if message.is(<PyGamepadRumbleRequest as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::GamepadRumbleRequest));
        }

        if message.is(<PyWindowEvent as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::WindowEvent));
        }

        if message.is(<PyWorldInstanceReady as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::WorldInstanceReady));
        }

        // AssetEvent is generic over the asset channel. The bare class cannot
        // identify which Bevy `Messages<AssetEvent<A>>` resource to access.
        use crate::assets::asset_event::PyAssetEvent;
        if message.is(<PyAssetEvent as PyTypeInfo>::type_object(py)) {
            return Err(PyTypeError::new_err(ASSET_EVENT_TYPE_REQUIRED));
        }

        if message.is(<PyAssetLoadFailedEvent as PyTypeInfo>::type_object(py)) {
            return Err(PyTypeError::new_err(ASSET_LOAD_FAILED_TYPE_REQUIRED));
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

#[pyclass(name = "MessageTypeParam", module = "pybevy.ecs", frozen)]
#[derive(Debug, PartialEq)]
pub struct PyMessageTypeParam(pub(crate) PyMessageType);

#[pymethods]
impl PyMessageTypeParam {
    /// Report a custom class held by the `Messages[T]` annotation parameter.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        self.0.0.traverse(&visit)
    }
}

/// Insert empty buffers for built-in message types whose owning plugin may be
/// absent, keeping reader/writer resource access consistent. Registered in
/// PreStartup so any plugin that owns a buffer (and its double-buffering update
/// system) has already inserted it first; this only fills genuinely missing ones.
pub(crate) fn ensure_builtin_message_resources(world: &mut World) {
    ensure_message_resource::<KeyboardInput>(world);
    ensure_message_resource::<WindowEvent>(world);
    ensure_message_resource::<WindowResized>(world);
    ensure_message_resource::<WindowCreated>(world);
    ensure_message_resource::<WindowScaleFactorChanged>(world);
    ensure_message_resource::<WindowClosing>(world);
    ensure_message_resource::<WorldInstanceReadyMessage>(world);
}
