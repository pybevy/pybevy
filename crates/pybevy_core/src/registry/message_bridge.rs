//! Message bridge trait for runtime type dispatch
//!
//! This module provides the `MessageBridge` trait that allows feature crates
//! to register their Bevy message types without the core crate needing to import them.
//!
//! # Pattern
//!
//! 1. Feature crate implements `MessageBridge` for each message type
//! 2. Feature crate's plugin registers bridges at startup
//! 3. Core uses bridges via runtime dispatch (no compile-time coupling)
//!
//! This replaces the hard-coded if-chain in `PyMessageType::from_message_type()`
//! with dynamic dispatch via the global registry.

use std::any::{Any, TypeId};

use bevy::ecs::{
    component::ComponentId,
    message::{Message, MessageCursor, MessageRegistry, Messages},
    world::World,
};
use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

/// A `MessageReader` operation on the reader's own cursor, as in Bevy's
/// `MessageCursor`; the shared `Messages<M>` buffer is never modified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderCursorOp {
    /// Count the messages this reader has not read (`MessageReader::len`).
    UnreadLen,
    /// Mark every buffered message as read by this reader (`MessageReader::clear`).
    Clear,
}

/// Apply `op` to a reader's type-erased persistent cursor and return how many
/// messages it had not read beforehand. A missing buffer reads as empty.
pub fn apply_reader_cursor<M: Message>(
    world: &World,
    cursor_state: &mut Option<Box<dyn Any + Send + Sync>>,
    op: ReaderCursorOp,
) -> usize {
    apply_cursor_op(world.get_resource::<Messages<M>>(), cursor_state, op)
}

/// [`apply_reader_cursor`] over a buffer the caller already resolved, such as
/// one of `RemovedComponentMessages`' per-component buffers.
pub fn apply_cursor_op<M: Message>(
    messages: Option<&Messages<M>>,
    cursor_state: &mut Option<Box<dyn Any + Send + Sync>>,
    op: ReaderCursorOp,
) -> usize {
    let Some(messages) = messages else {
        return 0;
    };
    let mut cursor = cursor_state
        .as_ref()
        .and_then(|state| state.downcast_ref::<MessageCursor<M>>())
        .cloned()
        .unwrap_or_else(|| messages.get_cursor());
    let unread = cursor.len(messages);
    if op == ReaderCursorOp::Clear {
        cursor.clear(messages);
        *cursor_state = Some(Box::new(cursor));
    }
    unread
}

/// Register a native channel once, preserving any plugin-owned buffer.
pub fn register_native_message<M: Message>(world: &mut World) {
    if !world.contains_resource::<Messages<M>>() {
        MessageRegistry::register_message::<M>(world);
    }
}

/// Trait that bridges a Bevy message type to its Python wrapper.
///
/// Each feature crate implements this for its message types. The trait provides
/// all the methods needed for:
/// - Type identification (Rust TypeId, Python type object)
/// - Message iteration (reading from Messages<T>)
/// - Message writing (if applicable)
///
/// # Read-Only Messages
///
/// A bridge is read-only unless its wrapper opts in with `#[pymessage(T, writable)]`,
/// which requires `TryFrom<&PyT> for T`. Messages Bevy itself lets user code send,
/// such as the keyboard, mouse and gamepad input messages, opt in; messages the
/// engine only reports (window events, for example) stay read-only.
pub trait MessageBridge: Send + Sync + 'static {
    /// Rust TypeId of the Bevy message type
    fn bevy_type_id(&self) -> TypeId;

    /// Python type object pointer for type matching
    ///
    /// Used for O(1) lookup in HashMap when dispatching from Python types.
    fn py_type_ptr(&self) -> *const PyTypeObject;

    /// Get Python type object
    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType>;

    /// Human-readable name for error messages
    fn name(&self) -> &'static str;

    /// Whether this message type is read-only (cannot be written from Python)
    ///
    /// Returns false for wrappers declared `writable`; true otherwise.
    fn is_read_only(&self) -> bool {
        true // Default to read-only for safety
    }

    /// Iterate messages from the world and convert to Python objects.
    /// Creates a fresh cursor each time (reads all buffered messages).
    fn iter_to_python(&self, py: Python, world: &mut World) -> PyResult<Vec<Py<PyAny>>>;

    /// Iterate messages using a persistent cursor to avoid re-reading.
    ///
    /// The cursor state is stored in `cursor_storage` between calls.
    /// On first call (when cursor_storage is None), reads all buffered messages.
    /// On subsequent calls, only reads messages written since the last read.
    ///
    /// Default implementation falls back to `iter_to_python` (fresh cursor).
    fn iter_to_python_with_cursor(
        &self,
        py: Python,
        world: &mut World,
        cursor_storage: &mut Option<Box<dyn Any + Send + Sync>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let _ = cursor_storage; // Default ignores cursor storage
        self.iter_to_python(py, world)
    }

    /// Write a message to the world
    fn write_message(
        &self,
        _py: Python,
        _world: &mut World,
        _message: &Bound<'_, PyAny>,
    ) -> PyResult<u64> {
        Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{} messages are read-only (generated by system)",
            self.name()
        )))
    }

    /// Apply a reader cursor operation; see [`apply_reader_cursor`].
    fn reader_cursor(
        &self,
        world: &World,
        cursor_state: &mut Option<Box<dyn Any + Send + Sync>>,
        op: ReaderCursorOp,
    ) -> usize;

    /// Clear all messages of this type
    fn clear(&self, world: &mut World) -> PyResult<()>;

    /// Check if there are no messages of this type
    fn is_empty(&self, world: &mut World) -> PyResult<bool>;

    /// Get the number of messages of this type
    fn len(&self, world: &mut World) -> PyResult<usize>;

    /// Get the ComponentId for the Messages<T> resource in the world
    ///
    /// Used by system access declaration to register read/write access.
    fn resource_id(&self, world: &World) -> Option<ComponentId>;

    /// Initialize the native buffer and its Bevy message maintenance once.
    fn register_messages(&self, world: &mut World);

    /// Register (get-or-create) the ComponentId for the `Messages<T>` resource.
    ///
    /// Unlike `resource_id`, this never returns None: it creates the id when the
    /// `Messages<T>` resource is absent so `DynamicSystem::initialize` can declare
    /// access even before the message type's first write. The created id is
    /// TypeId-keyed and equals the one a later insertion resolves to.
    fn register_resource_id(&self, world: &mut World) -> ComponentId;
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{
        any::{Any, TypeId},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use bevy::ecs::{component::ComponentId, message::Message};
    use pyo3::{
        exceptions::PyTypeError,
        ffi::PyTypeObject,
        prelude::*,
        types::{PyInt, PyType},
    };

    use super::*;

    const FAKE_PTR: usize = 0x7201;
    static FRESH_READS: AtomicUsize = AtomicUsize::new(0);

    // Implements only the required methods: is_read_only, iter_to_python_with_cursor
    // and write_message stay on their trait defaults.
    struct DefaultMessageBridge {
        name: &'static str,
    }

    impl MessageBridge for DefaultMessageBridge {
        fn register_messages(&self, _world: &mut World) {
            unreachable!("default-methods-only fake bridge")
        }

        fn bevy_type_id(&self) -> TypeId {
            TypeId::of::<DefaultMessageBridge>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            FAKE_PTR as *const PyTypeObject
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!("default-methods-only fake bridge")
        }

        fn name(&self) -> &'static str {
            self.name
        }

        fn iter_to_python(&self, py: Python, _world: &mut World) -> PyResult<Vec<Py<PyAny>>> {
            let read = FRESH_READS.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(vec![PyInt::new(py, read).into()])
        }

        fn reader_cursor(
            &self,
            _world: &World,
            _cursor_state: &mut Option<Box<dyn Any + Send + Sync>>,
            _op: ReaderCursorOp,
        ) -> usize {
            0
        }

        fn clear(&self, _world: &mut World) -> PyResult<()> {
            Ok(())
        }

        fn is_empty(&self, _world: &mut World) -> PyResult<bool> {
            Ok(true)
        }

        fn len(&self, _world: &mut World) -> PyResult<usize> {
            Ok(0)
        }

        fn resource_id(&self, _world: &World) -> Option<ComponentId> {
            None
        }

        fn register_resource_id(&self, _world: &mut World) -> ComponentId {
            ComponentId::new(0)
        }
    }

    #[test]
    fn default_is_read_only_is_true() {
        let bridge = DefaultMessageBridge {
            name: "DefaultFakeMessage",
        };
        assert!(
            bridge.is_read_only(),
            "un-overridden bridge must default to read-only"
        );
    }

    #[test]
    fn default_write_message_reports_the_bridge_name() {
        let bridge = DefaultMessageBridge {
            name: "DefaultFakeMessage",
        };

        Python::attach(|py| {
            let mut world = World::new();
            let message = py.None();
            let error = bridge
                .write_message(py, &mut world, message.bind(py))
                .unwrap_err();
            assert!(
                error.is_instance_of::<PyTypeError>(py),
                "the default write_message must raise TypeError"
            );
            // Pin the exact user-visible message (Python `str(exc)`).
            assert_eq!(
                error.value(py).str().unwrap().to_string(),
                "DefaultFakeMessage messages are read-only (generated by system)"
            );
        });
    }

    #[test]
    fn default_cursor_iteration_falls_back_to_the_fresh_cursor() {
        let bridge = DefaultMessageBridge {
            name: "DefaultFakeMessage",
        };
        let before = FRESH_READS.load(Ordering::SeqCst);
        let mut storage: Option<Box<dyn Any + Send + Sync>> = None;

        Python::attach(|py| {
            let mut world = World::new();
            let first = bridge
                .iter_to_python_with_cursor(py, &mut world, &mut storage)
                .unwrap();
            let second = bridge
                .iter_to_python_with_cursor(py, &mut world, &mut storage)
                .unwrap();
            assert_eq!(first.len(), 1);
            assert_eq!(second.len(), 1);
            // Each call performs one fresh read: the counter-backed values advance.
            assert_eq!(
                first[0].extract::<i64>(py).unwrap(),
                i64::try_from(before).unwrap() + 1
            );
            assert_eq!(
                second[0].extract::<i64>(py).unwrap(),
                i64::try_from(before).unwrap() + 2
            );
        });

        assert!(
            storage.is_none(),
            "the default leaves cursor storage untouched"
        );
        assert_eq!(FRESH_READS.load(Ordering::SeqCst), before + 2);
    }

    #[derive(Message)]
    struct Ping;

    #[test]
    fn clearing_one_reader_cursor_leaves_the_buffer_and_other_readers() {
        let mut world = World::new();
        world.init_resource::<Messages<Ping>>();
        for _ in 0..3 {
            world.write_message(Ping);
        }
        let mut cleared = None;
        let mut other = None;

        assert_eq!(
            apply_reader_cursor::<Ping>(&world, &mut cleared, ReaderCursorOp::Clear),
            3
        );
        assert_eq!(
            apply_reader_cursor::<Ping>(&world, &mut cleared, ReaderCursorOp::UnreadLen),
            0
        );
        assert_eq!(
            apply_reader_cursor::<Ping>(&world, &mut other, ReaderCursorOp::UnreadLen),
            3
        );
        assert!(other.is_none(), "counting leaves an unset cursor unset");
        assert_eq!(world.resource::<Messages<Ping>>().len(), 3);

        world.write_message(Ping);
        assert_eq!(
            apply_reader_cursor::<Ping>(&world, &mut cleared, ReaderCursorOp::UnreadLen),
            1
        );
    }

    #[test]
    fn a_missing_buffer_reads_as_empty() {
        let world = World::new();
        let mut cursor = None;
        assert_eq!(
            apply_reader_cursor::<Ping>(&world, &mut cursor, ReaderCursorOp::Clear),
            0
        );
        assert!(cursor.is_none());
    }
}
