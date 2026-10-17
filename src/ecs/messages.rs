use std::{
    any::Any,
    mem::transmute,
    ptr::eq,
    sync::{Arc, Mutex, MutexGuard},
};

use bevy::{
    ecs::{
        component::ComponentId,
        message::{Message, Messages},
        world::{World, unsafe_world_cell::UnsafeWorldCell},
    },
    input::keyboard::KeyboardInput,
    window::{WindowClosing, WindowCreated, WindowEvent, WindowResized, WindowScaleFactorChanged},
};
use pybevy_core::{
    ReaderCursorOp, apply_reader_cursor,
    public_error::{
        ASSET_BRIDGE_NOT_FOUND, ASSET_EVENT_TYPE_REQUIRED, ASSET_LOAD_FAILED_TYPE_REQUIRED,
        MESSAGES_RESOURCE_READ_ONLY,
    },
    read_next_message,
    registry::{global_registry, register_native_message},
};
use pybevy_ecs::shared::message_resources::ensure_message_resource;
use pybevy_world_serialization::{
    WorldInstanceReadyMessage, world_instance_ready::PyWorldInstanceReady,
};
use pyo3::{
    IntoPyObjectExt, PyTraverseError, PyTypeInfo, PyVisit,
    exceptions::{PyNotImplementedError, PyRuntimeError, PyTypeError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyTuple, PyType, PyTypeMethods},
};

use super::{message::PyMessage, python_message::register_python_message};
use crate::{
    assets::{
        asset_event::{PyAssetEvent, materialize_asset_event_record},
        asset_load_failed_event::{PyAssetLoadFailedEvent, materialize_asset_load_failed_record},
    },
    ecs::{
        deferred_drop::WorldMutGuard, dynamic_system::lock_or_recover,
        helpers::validity_guard::ValidityFlag, resource::PyResource, world_gc::WorldGcState,
    },
};

/// Persistent cursor storage shared between DynamicSystem and PyMessages.
/// The Arc+Mutex allows frozen pyclass fields to hold mutable state.
pub(crate) type CursorStorage = Arc<Mutex<Option<Box<dyn Any + Send + Sync>>>>;

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
    gc_state: Option<WorldGcState>,
}

// SAFETY: mirrors PyQueryIter's discipline. The cell is only touched while the
// owning system runs on a single thread, fenced by `validity`.
unsafe impl Send for MessageWorld {}
// SAFETY: shared access checks the same owning-thread validity and scheduler permissions.
unsafe impl Sync for MessageWorld {}

impl MessageWorld {
    /// # Safety
    /// `cell` must reference the world holding the message buffers and stay valid
    /// for as long as `validity` is active.
    pub(crate) unsafe fn new(cell: UnsafeWorldCell, validity: ValidityFlag) -> Self {
        // SAFETY: layout-preserving lifetime erasure of a Copy pointer type; the
        // cell is only touched while `validity` is active.
        let cell: UnsafeWorldCell<'static> = unsafe { transmute(cell) };
        Self {
            cell,
            validity,
            gc_state: WorldGcState::for_world(cell.id()),
        }
    }

    /// Momentary `&mut World` for the message macros/bridges (they take `&mut World`
    /// but only touch the declared `Messages<T>` resource).
    ///
    /// SAFETY of the returned reference: `initialize` declares reads for
    /// reader resource ids and writes for writer ids; the executor prevents a
    /// conflicting system running concurrently, so the actual message access is
    /// unique. Same residual-pointer class as `query_runtime::world_ptr`.
    pub(crate) fn world_mut(&self) -> PyResult<WorldMutGuard<'_>> {
        self.validity.check()?;
        let gc = self.gc_state.as_ref().map(WorldGcState::suspend);
        // SAFETY: traversal is suspended; see the validity/access contract above.
        Ok(WorldMutGuard::with_gc(unsafe { self.cell.world_mut() }, gc))
    }
}

/// Python-facing wrapper for Bevy message resources (events).
///
/// Provides read and write access to a specific message type's resource,
/// used by `MessageReader` and `MessageWriter` system parameters.
#[pyclass(name = "Messages", module = "pybevy.ecs", extends = PyResource, frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct PyMessages {
    pub(crate) message_type: MessageType,
    pub(crate) world: MessageWorld,
    /// Persistent cursor for MessageReader iteration.
    /// When Some, each yielded message advances the stored cursor.
    /// When None, creates a fresh cursor each time (legacy behavior).
    pub(crate) cursor_storage: Option<CursorStorage>,
    /// Set for `World.resource()`, whose `&Messages<T>` cannot be cleared.
    pub(crate) read_only: bool,
}

impl PyMessages {
    pub(crate) fn check_valid(&self) -> PyResult<()> {
        self.world
            .validity
            .check_read()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    pub(crate) fn next_to_python(&self, py: Python) -> PyResult<Option<Py<PyAny>>> {
        let world = self.world.world_mut()?;
        let mut guard = self.lock_cursor();
        let mut detached = None;
        let state = match guard.as_mut() {
            Some(state) => &mut **state,
            None => &mut detached,
        };
        match &self.message_type {
            MessageType::WorldInstanceReady => read_next_message(
                world.get_resource::<Messages<WorldInstanceReadyMessage>>(),
                state,
                |event| {
                    Py::new(py, (PyWorldInstanceReady::from(&event.0), PyMessage)).map(Py::into_any)
                },
            ),
            MessageType::AssetEvent(type_ptr) => {
                let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
                bridge
                    .read_next_event(&world, state)
                    .map(|event| materialize_asset_event_record(py, event))
                    .transpose()
            }
            MessageType::AssetLoadFailed(type_ptr) => {
                let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
                bridge
                    .read_next_load_failed_event(&world, state)
                    .map(|event| materialize_asset_load_failed_record(py, event))
                    .transpose()
            }
            MessageType::Dynamic(type_ptr) => {
                let bridge =
                    global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                        PyTypeError::new_err("Message type not registered in global registry")
                    })?;
                bridge.next_to_python(py, &world, state)
            }
            MessageType::Custom(_) => Err(PyTypeError::new_err(
                "custom messages use the shared Python message store",
            )),
        }
    }

    /// Apply a `MessageReader` cursor operation and return the reader's unread
    /// count before it. Only the reader's cursor changes, as in Bevy.
    pub(crate) fn reader_cursor(&self, op: ReaderCursorOp) -> PyResult<usize> {
        let world = self.world.world_mut()?;
        let world: &World = &world;
        let mut guard = self.lock_cursor();
        let mut detached = None;
        let cursor_state = match guard.as_mut() {
            Some(state) => &mut **state,
            None => &mut detached,
        };
        Ok(match &self.message_type {
            MessageType::WorldInstanceReady => {
                apply_reader_cursor::<WorldInstanceReadyMessage>(world, cursor_state, op)
            }
            MessageType::AssetEvent(type_ptr) => {
                global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?
                    .event_reader_cursor(world, cursor_state, op)
            }
            MessageType::AssetLoadFailed(type_ptr) => {
                global_registry::get_asset_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?
                    .load_failed_reader_cursor(world, cursor_state, op)
            }
            MessageType::Custom(_) => {
                return Err(PyTypeError::new_err(
                    "custom messages use the shared Python message store",
                ));
            }
            MessageType::Dynamic(type_ptr) => {
                global_registry::get_message_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| {
                        PyTypeError::new_err("Message type not registered in global registry")
                    })?
                    .reader_cursor(world, cursor_state, op)
            }
        })
    }

    /// Lock cursor storage (if available) and return a mutable reference to the inner state.
    /// Returns a default None state when no cursor storage is configured.
    fn lock_cursor(&self) -> Option<MutexGuard<'_, Option<Box<dyn Any + Send + Sync>>>> {
        self.cursor_storage.as_ref().map(|cs| lock_or_recover(cs))
    }

    fn with_messages<F, R>(&self, f: F) -> PyResult<R>
    where
        F: FnOnce(&mut dyn ErasedMessages) -> R,
    {
        let mut world = self.world.world_mut()?;

        // clear/is_empty/len back these arms; a missing buffer reads as empty
        // (EmptyMessages) rather than inserting a resource, which would be a
        // structural world mutation from a possibly-parallel system.
        match &self.message_type {
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
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn iter_messages(&self, py: Python) -> PyResult<Vec<Py<PyAny>>>;
}

struct Wrap<'a, T: Message>(pub &'a mut Messages<T>);

impl<'a, T: Message> ErasedMessages for Wrap<'a, T> {
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
        Err(PyNotImplementedError::new_err(
            "iter_messages not implemented for this message type",
        ))
    }
}

/// Stand-in for an absent `Messages<T>` resource. Reader-side maintenance
/// (clear/is_empty/len) treats a missing buffer as empty instead of inserting one.
struct EmptyMessages;

impl ErasedMessages for EmptyMessages {
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

    pub fn clear(&self) -> PyResult<()> {
        if self.read_only {
            return Err(PyRuntimeError::new_err(MESSAGES_RESOURCE_READ_ONLY));
        }
        if let MessageType::AssetEvent(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return {
                let _: () = bridge.clear_events(&mut *self.world.world_mut()?);
                Ok(())
            };
        }
        if let MessageType::AssetLoadFailed(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            bridge.clear_load_failed_events(&mut *self.world.world_mut()?);
            return Ok(());
        }
        if let MessageType::Dynamic(type_ptr) = &self.message_type {
            let bridge =
                global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                    PyTypeError::new_err("Message type not registered in global registry")
                })?;
            let mut world = self.world.world_mut()?;
            return bridge.clear(&mut world);
        }
        self.with_messages(|messages| messages.clear())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        if let MessageType::AssetEvent(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return Ok(bridge.events_is_empty(&*self.world.world_mut()?));
        }
        if let MessageType::AssetLoadFailed(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return Ok(bridge.load_failed_events_is_empty(&*self.world.world_mut()?));
        }
        if let MessageType::Dynamic(type_ptr) = &self.message_type {
            let bridge =
                global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                    PyTypeError::new_err("Message type not registered in global registry")
                })?;
            let mut world = self.world.world_mut()?;
            return bridge.is_empty(&mut world);
        }
        self.with_messages(|messages| messages.is_empty())
    }

    fn __len__(&self) -> PyResult<usize> {
        self.len()
    }

    pub fn len(&self) -> PyResult<usize> {
        if let MessageType::AssetEvent(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return Ok(bridge.event_count(&*self.world.world_mut()?));
        }
        if let MessageType::AssetLoadFailed(type_ptr) = &self.message_type {
            let bridge = global_registry::get_asset_bridge_by_py_type(*type_ptr)
                .ok_or_else(|| PyTypeError::new_err(ASSET_BRIDGE_NOT_FOUND))?;
            return Ok(bridge.load_failed_event_count(&*self.world.world_mut()?));
        }
        if let MessageType::Dynamic(type_ptr) = &self.message_type {
            let bridge =
                global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                    PyTypeError::new_err("Message type not registered in global registry")
                })?;
            let mut world = self.world.world_mut()?;
            return bridge.len(&mut world);
        }
        self.with_messages(|messages| messages.len())
    }
}

#[derive(Debug)]
pub enum MessageType {
    // Special handling required (no bridge)
    WorldInstanceReady,
    AssetEvent(*const PyTypeObject),
    AssetLoadFailed(*const PyTypeObject),
    // Python custom messages
    Custom(Py<PyType>),
    /// Dynamic message type registered via global message bridge registry.
    /// All event types with #[pymessage] use this variant.
    Dynamic(*const PyTypeObject),
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

// SAFETY: holders retain custom message classes; native classes are module-owned.
unsafe impl Send for MessageType {}
// SAFETY: dereferencing a retained class requires an attached Python thread.
unsafe impl Sync for MessageType {}

impl PartialEq for MessageType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MessageType::WorldInstanceReady, MessageType::WorldInstanceReady) => true,
            (MessageType::AssetEvent(a), MessageType::AssetEvent(b)) => eq(*a, *b),
            (MessageType::AssetLoadFailed(a), MessageType::AssetLoadFailed(b)) => eq(*a, *b),
            (MessageType::Custom(a), MessageType::Custom(b)) => a.is(b),
            (MessageType::Dynamic(a), MessageType::Dynamic(b)) => eq(*a, *b),
            _ => false,
        }
    }
}

impl Clone for MessageType {
    fn clone(&self) -> Self {
        match self {
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

    /// Display name of the `Messages<T>` resource for this message type.
    pub(crate) fn resource_name(&self) -> String {
        let (_, label) = self.validation_identity();
        label
            .strip_prefix("Message<")
            .map_or(label.clone(), |inner| format!("Messages<{inner}"))
    }

    /// Register (get-or-create) the ComponentId for a native `Messages<T>` resource.
    ///
    /// `DynamicSystem::initialize` calls this before the first write so native
    /// message access is never silently omitted. Custom Python messages declare
    /// their shared-store and synthetic channel access in `MainResolver` instead.
    pub(crate) fn register_resource_id(&self, world: &mut World) -> Option<ComponentId> {
        match self {
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

    /// Whether `message` or one of its bases is a Python or native message type.
    pub(crate) fn is_message_class(message: &Bound<'_, PyType>) -> PyResult<bool> {
        if message.is_subclass_of::<PyMessage>()? {
            return Ok(true);
        }
        let mro = message.getattr("__mro__")?.cast_into::<PyTuple>()?;
        for base in mro.iter() {
            if Self::from_message_type(&base.cast_into::<PyType>()?).is_ok() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn from_message_type(message: &Bound<'_, PyType>) -> PyResult<Self> {
        let py = message.py();
        let type_ptr = message.as_type_ptr();

        // Check global message bridge registry first (dynamic dispatch)
        // Most message types use this path via #[pymessage] attribute
        if global_registry::contains_message_py_type(type_ptr) {
            return Ok(PyMessageType(MessageType::Dynamic(type_ptr)));
        }

        // Enum-backed message instances use nested variant classes, while the
        // bridge is registered on their common message base class.
        let mro = message.getattr("__mro__")?.cast_into::<PyTuple>()?;
        for base in mro.iter().skip(1) {
            let base = base.cast_into::<PyType>()?;
            let base_ptr = base.as_type_ptr();
            if global_registry::contains_message_py_type(base_ptr) {
                return Ok(PyMessageType(MessageType::Dynamic(base_ptr)));
            }
        }

        // Special handling types that don't use #[pymessage] (need extra resources/special logic)
        if message.is(<PyWorldInstanceReady as PyTypeInfo>::type_object(py)) {
            return Ok(PyMessageType(MessageType::WorldInstanceReady));
        }

        // AssetEvent is generic over the asset channel. The bare class cannot
        // identify which Bevy `Messages<AssetEvent<A>>` resource to access.
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

/// Register the buffer selected by the same dispatch used by system parameters.
pub(crate) fn register_message_channel(
    py: Python,
    world: &mut World,
    message_type: &Bound<'_, PyType>,
    generation: u32,
) -> PyResult<()> {
    match PyMessageType::from_message_type(message_type)?.0 {
        MessageType::Custom(_) => {
            register_python_message(py, world, message_type, generation)?;
        }
        MessageType::Dynamic(type_ptr) => {
            global_registry::get_message_bridge_by_py_type(type_ptr)
                .ok_or_else(|| {
                    PyTypeError::new_err("Message type not registered in global registry")
                })?
                .register_messages(world);
        }
        MessageType::WorldInstanceReady => {
            register_native_message::<WorldInstanceReadyMessage>(world)
        }
        MessageType::AssetEvent(_) | MessageType::AssetLoadFailed(_) => {
            unreachable!("asset channels require a parameterized annotation")
        }
    }
    Ok(())
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
