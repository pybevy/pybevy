use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use pybevy_core::registry::global_registry;
// Re-export base classes from pybevy_core
pub use pybevy_core::{PyMessage, PyMessageId};
use pybevy_ecs::shared::{
    message_store::{
        MessageConsumeOutcome, MessageRecord, MessageStoreError, SharedMessageCursor,
        new_message_cursor,
    },
    parity_trace::{ParityOpKind, ParityRunHandle, PendingParityOp},
};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyRuntimeError, PyStopIteration, PyTypeError},
    prelude::*,
    types::PyType,
};

use super::{
    messages::{MessageType, MessageWorld, PyMessages},
    python_message::{PythonMessageValue, ResolvedPythonMessage, resolve_from_world},
    world::PyWorld,
};
use crate::ecs::{
    helpers::validity_guard::ValidityFlag,
    messages::{CursorStorage, PyMessageType},
    parity_trace::canonicalize_payload,
};

fn store_error(error: MessageStoreError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

pub(crate) fn python_message_cursor(
    storage: Option<CursorStorage>,
) -> PyResult<SharedMessageCursor> {
    let Some(storage) = storage else {
        return Ok(new_message_cursor());
    };
    let mut state = storage
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(existing) = state.as_ref() {
        return existing
            .downcast_ref::<SharedMessageCursor>()
            .cloned()
            .ok_or_else(|| PyRuntimeError::new_err("message cursor state has the wrong type"));
    }
    let cursor = new_message_cursor();
    *state = Some(Box::new(Arc::clone(&cursor)));
    Ok(cursor)
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageClass {
    Reader,
    Writer,
    Mutator,
}

#[pyclass(frozen, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct MessageTypeParam {
    pub(crate) ty: MessageClass,
    pub(crate) message_type: MessageType,
}

/// Write-only handle to a message channel. Used as a system parameter to send messages.
///
/// Usage: `def my_system(writer: MessageWriter[DamageEvent])`
#[pyclass(name = "MessageWriter", frozen)]
pub struct PyMessageWriter {
    pub(crate) message_type: MessageType,
    world: Option<MessageWorld>,
    python: Option<PythonMessageWriter>,
    parity_trace: Option<ParityRunHandle>,
}

impl PyMessageWriter {
    pub(crate) fn native(
        message_type: MessageType,
        world: MessageWorld,
        parity_trace: Option<ParityRunHandle>,
    ) -> Self {
        Self {
            message_type,
            world: Some(world),
            python: None,
            parity_trace,
        }
    }

    pub(crate) fn python(
        message_type: MessageType,
        resolved: ResolvedPythonMessage,
        validity: ValidityFlag,
        parity_trace: Option<ParityRunHandle>,
    ) -> Self {
        Self {
            message_type,
            world: None,
            python: Some(PythonMessageWriter { resolved, validity }),
            parity_trace,
        }
    }

    fn native_world(&self) -> PyResult<&MessageWorld> {
        self.world.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("native message writer is missing its World access")
        })
    }

    fn python_writer(&self) -> PyResult<&PythonMessageWriter> {
        self.python.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("custom message writer is missing its store context")
        })
    }

    fn prepare_trace_op(&self, message: &Bound<'_, PyAny>) -> PyResult<Option<PendingParityOp>> {
        if self.parity_trace.is_none() {
            return Ok(None);
        }
        Ok(Some(PendingParityOp {
            kind: ParityOpKind::MessageWrite,
            type_name: Some(message.get_type().name()?.to_string()),
            payload_digest: canonicalize_payload(message)?.digest(),
            target: None,
        }))
    }

    fn record_trace_op(&self, operation: Option<PendingParityOp>) {
        if let (Some(trace), Some(operation)) = (&self.parity_trace, operation) {
            trace.record_op(operation);
        }
    }
}

struct PythonMessageWriter {
    resolved: ResolvedPythonMessage,
    validity: ValidityFlag,
}

#[pymethods]
impl PyMessageWriter {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = key.py();

        // Extract message type from key (e.g., AppExit from MessageWriter[AppExit])
        let type_obj = key.cast::<PyType>()?;
        let py_message_type = PyMessageType::from_message_type(type_obj)?;

        // Create MessageTypeParam with Writer class
        let param = MessageTypeParam {
            ty: MessageClass::Writer,
            message_type: py_message_type.0,
        };

        param.into_py_any(py)
    }

    pub fn write(&self, py: Python, message: Py<PyAny>) -> PyResult<PyMessageId> {
        let bound_message = message.bind(py);

        match &self.message_type {
            MessageType::KeyboardInput => Err(PyTypeError::new_err(
                "KeyboardInput events are read-only (generated by input system)",
            )),
            MessageType::GamepadRumbleRequest => Err(PyTypeError::new_err(
                "GamepadRumbleRequest is write-only and not accessible through MessageReader or MessageWriter",
            )),
            MessageType::WindowEvent => Err(PyTypeError::new_err(
                "WindowEvent events are read-only (generated by window system)",
            )),
            MessageType::WorldInstanceReady => Err(PyTypeError::new_err(
                "WorldInstanceReady events are read-only (generated by world serialization system)",
            )),
            MessageType::AssetEventImage => Err(PyTypeError::new_err(
                "AssetEvent<Image> not yet implemented",
            )),
            MessageType::AssetEventMesh => {
                Err(PyTypeError::new_err("AssetEvent<Mesh> not yet implemented"))
            }
            MessageType::Custom(_) => {
                let trace_operation = self.prepare_trace_op(bound_message)?;
                let writer = self.python_writer()?;
                writer
                    .validity
                    .check_read()
                    .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
                // Verify the message is an instance of the registered type
                let msg_type = bound_message.get_type();
                let expected_type = writer.resolved.class.bind(py);
                if !msg_type.is(expected_type) {
                    return Err(PyTypeError::new_err(format!(
                        "Expected message of type {}, got {}",
                        expected_type.name()?,
                        msg_type.name()?,
                    )));
                }

                let id = writer
                    .resolved
                    .store
                    .append(writer.resolved.channel, Arc::new(message))
                    .map_err(store_error)?;
                self.record_trace_op(trace_operation);
                Ok(PyMessageId::new(id))
            }
            MessageType::Dynamic(type_ptr) => {
                let trace_operation = self.prepare_trace_op(bound_message)?;
                // Use global registry for dynamic dispatch (most message types use this)
                let bridge =
                    global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                        PyTypeError::new_err("Message type not registered in global registry")
                    })?;

                if bridge.is_read_only() {
                    return Err(PyTypeError::new_err(format!(
                        "{} messages are read-only (generated by system)",
                        bridge.name()
                    )));
                }

                let world = self.native_world()?.world_mut()?;
                let event_id = bridge.write_message(py, world, bound_message)?;
                self.record_trace_op(trace_operation);
                Ok(PyMessageId::from_boxed(event_id))
            }
        }
    }

    pub fn write_batch(&self, py: Python, messages: Vec<Py<PyAny>>) -> PyResult<Vec<PyMessageId>> {
        if matches!(self.message_type, MessageType::Custom(_)) {
            let trace_operations = messages
                .iter()
                .map(|message| self.prepare_trace_op(message.bind(py)))
                .collect::<PyResult<Vec<_>>>()?;
            let writer = self.python_writer()?;
            writer
                .validity
                .check_read()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let expected_type = writer.resolved.class.bind(py);
            for message in &messages {
                let actual_type = message.bind(py).get_type();
                if !actual_type.is(expected_type) {
                    return Err(PyTypeError::new_err(format!(
                        "Expected message of type {}, got {}",
                        expected_type.name()?,
                        actual_type.name()?,
                    )));
                }
            }
            let values = messages.into_iter().map(Arc::new).collect();
            let ids = writer
                .resolved
                .store
                .append_batch(writer.resolved.channel, values)
                .map_err(store_error)
                .map(|ids| ids.into_iter().map(PyMessageId::new).collect())?;
            for operation in trace_operations {
                self.record_trace_op(operation);
            }
            return Ok(ids);
        }
        let mut message_ids = Vec::new();
        for message in messages {
            let message_id = self.write(py, message)?;
            message_ids.push(message_id);
        }
        Ok(message_ids)
    }

    pub fn write_default(&self, py: Python) -> PyResult<PyMessageId> {
        match &self.message_type {
            MessageType::KeyboardInput => Err(PyTypeError::new_err(
                "KeyboardInput events are read-only (generated by input system)",
            )),
            MessageType::GamepadRumbleRequest => Err(PyTypeError::new_err(
                "GamepadRumbleRequest is write-only and not accessible through MessageReader or MessageWriter",
            )),
            MessageType::WindowEvent => Err(PyTypeError::new_err(
                "WindowEvent events are read-only (generated by window system)",
            )),
            MessageType::WorldInstanceReady => Err(PyTypeError::new_err(
                "WorldInstanceReady events are read-only (generated by world serialization system)",
            )),
            MessageType::AssetEventImage => Err(PyTypeError::new_err(
                "AssetEvent<Image> has no default value",
            )),
            MessageType::AssetEventMesh => Err(PyTypeError::new_err(
                "AssetEvent<Mesh> has no default value",
            )),
            MessageType::Custom(_) => {
                // Call the type with no arguments to create a default instance
                let writer = self.python_writer()?;
                writer
                    .validity
                    .check_read()
                    .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
                let instance = writer.resolved.class.bind(py).call0()?;
                self.write(py, instance.unbind())
            }
            MessageType::Dynamic(type_ptr) => {
                // Use global registry for dynamic dispatch
                let bridge =
                    global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                        PyTypeError::new_err("Message type not registered in global registry")
                    })?;

                if bridge.is_read_only() {
                    return Err(PyTypeError::new_err(format!(
                        "{} messages are read-only (generated by system)",
                        bridge.name()
                    )));
                }

                // Dynamic types don't support write_default - they need explicit values
                Err(PyTypeError::new_err(format!(
                    "{} messages don't support write_default",
                    bridge.name()
                )))
            }
        }
    }
}

/// Read-only handle to a message channel. Used as a system parameter to receive messages.
///
/// Usage: `def my_system(reader: MessageReader[DamageEvent])`
#[pyclass(name = "MessageReader", frozen)]
pub struct PyMessageReader {
    messages: Option<PyMessages>,
    python: Option<PythonMessageReader>,
}

impl PyMessageReader {
    pub(crate) fn native(messages: PyMessages) -> Self {
        Self {
            messages: Some(messages),
            python: None,
        }
    }

    pub(crate) fn python(
        resolved: ResolvedPythonMessage,
        cursor: SharedMessageCursor,
        validity: ValidityFlag,
    ) -> Self {
        Self {
            messages: None,
            python: Some(PythonMessageReader {
                resolved,
                cursor,
                validity,
            }),
        }
    }

    fn native_messages(&self) -> PyResult<&PyMessages> {
        self.messages.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("native message reader is missing its Messages access")
        })
    }

    fn python_reader(&self) -> Option<&PythonMessageReader> {
        self.python.as_ref()
    }
}

struct PythonMessageReader {
    resolved: ResolvedPythonMessage,
    cursor: SharedMessageCursor,
    validity: ValidityFlag,
}

impl PythonMessageReader {
    fn check(&self) -> PyResult<()> {
        self.validity
            .check_read()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }
}

#[pymethods]
impl PyMessageReader {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = key.py();

        // Extract message type from key (e.g., AppExit from MessageReader[AppExit])
        let type_obj = key.cast::<PyType>()?;
        let py_message_type = PyMessageType::from_message_type(type_obj)?;

        // Create MessageTypeParam with Reader class
        let param = MessageTypeParam {
            ty: MessageClass::Reader,
            message_type: py_message_type.0,
        };

        param.into_py_any(py)
    }

    pub fn clear(&self) -> PyResult<()> {
        if let Some(reader) = self.python_reader() {
            reader.check()?;
            return reader
                .resolved
                .store
                .clear_reader(reader.resolved.channel, &reader.cursor)
                .map_err(store_error);
        }
        self.native_messages()?.clear()
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        if let Some(reader) = self.python_reader() {
            reader.check()?;
            return reader
                .resolved
                .store
                .is_empty(reader.resolved.channel, &reader.cursor)
                .map_err(store_error);
        }
        self.native_messages()?.is_empty()
    }

    pub fn len(&self) -> PyResult<usize> {
        if let Some(reader) = self.python_reader() {
            reader.check()?;
            let len = reader
                .resolved
                .store
                .unread_len(reader.resolved.channel, &reader.cursor)
                .map_err(store_error)?;
            return usize::try_from(len)
                .map_err(|_| PyRuntimeError::new_err("unread message count exceeds usize"));
        }
        self.native_messages()?.len()
    }

    pub fn read(&self, py: Python) -> PyResult<PyMessageReaderIter> {
        if let Some(reader) = self.python_reader() {
            reader.check()?;
            let snapshot = reader
                .resolved
                .store
                .snapshot_unread(reader.resolved.channel, &reader.cursor)
                .map_err(store_error)?;
            return Ok(PyMessageReaderIter {
                cursor: AtomicUsize::new(0),
                cached_messages: Vec::new(),
                python: Some(PythonMessageIterator {
                    store: reader.resolved.store.clone(),
                    channel: reader.resolved.channel,
                    cursor: Arc::clone(&reader.cursor),
                    records: snapshot.records,
                    validity: reader.validity.clone(),
                }),
            });
        }

        // Native Bevy messages retain their existing eager adapter behavior.
        let cached_messages = self.native_messages()?.iter_to_python(py)?;

        Ok(PyMessageReaderIter {
            cursor: AtomicUsize::new(0),
            cached_messages,
            python: None,
        })
    }

    pub fn __iter__(&self, py: Python) -> PyResult<PyMessageReaderIter> {
        self.read(py)
    }
}

#[pyclass(name = "MessageReaderIter")]
pub struct PyMessageReaderIter {
    cursor: AtomicUsize,
    // Cache all messages at creation time for iteration
    cached_messages: Vec<Py<PyAny>>,
    python: Option<PythonMessageIterator>,
}

struct PythonMessageIterator {
    store: crate::ecs::python_message::PythonMessageStore,
    channel: pybevy_ecs::shared::message_store::MessageChannelId,
    cursor: SharedMessageCursor,
    records: Vec<MessageRecord<PythonMessageValue>>,
    validity: ValidityFlag,
}

#[pymethods]
impl PyMessageReaderIter {
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(&self, py: Python) -> PyResult<Py<PyAny>> {
        if let Some(iterator) = &self.python {
            iterator
                .validity
                .check_read()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            loop {
                let index = self.cursor.fetch_add(1, Ordering::Relaxed);
                let Some(record) = iterator.records.get(index) else {
                    return Err(PyStopIteration::new_err(""));
                };
                match iterator
                    .store
                    .consume_snapshot_record(iterator.channel, &iterator.cursor, record.sequence)
                    .map_err(store_error)?
                {
                    MessageConsumeOutcome::Consumed => {
                        return Ok(record.value.as_ref().clone_ref(py));
                    }
                    MessageConsumeOutcome::AlreadyConsumed => continue,
                }
            }
        }
        let index = self.cursor.fetch_add(1, Ordering::Relaxed);

        if index < self.cached_messages.len() {
            Ok(self.cached_messages[index].clone_ref(py))
        } else {
            Err(PyStopIteration::new_err(""))
        }
    }
}

/// Combined read/write handle for a custom Python message channel.
///
/// This is intentionally limited to custom messages: native bridges currently
/// materialize owned Python snapshots, which cannot provide Bevy's in-place
/// mutation semantics.
#[pyclass(name = "MessageMutator", frozen)]
pub struct PyMessageMutator {
    writer: PyMessageWriter,
    reader: PyMessageReader,
}

impl PyMessageMutator {
    pub(crate) fn python(
        message_type: MessageType,
        resolved: ResolvedPythonMessage,
        cursor: SharedMessageCursor,
        validity: ValidityFlag,
        parity_trace: Option<ParityRunHandle>,
    ) -> Self {
        Self {
            writer: PyMessageWriter::python(
                message_type,
                resolved.clone(),
                validity.clone(),
                parity_trace,
            ),
            reader: PyMessageReader::python(resolved, cursor, validity),
        }
    }
}

#[pymethods]
impl PyMessageMutator {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = key.py();
        let type_obj = key.cast::<PyType>()?;
        let py_message_type = PyMessageType::from_message_type(type_obj)?;
        MessageTypeParam {
            ty: MessageClass::Mutator,
            message_type: py_message_type.0,
        }
        .into_py_any(py)
    }

    pub fn write(&self, py: Python, message: Py<PyAny>) -> PyResult<PyMessageId> {
        self.writer.write(py, message)
    }

    pub fn write_batch(&self, py: Python, messages: Vec<Py<PyAny>>) -> PyResult<Vec<PyMessageId>> {
        self.writer.write_batch(py, messages)
    }

    pub fn write_default(&self, py: Python) -> PyResult<PyMessageId> {
        self.writer.write_default(py)
    }

    pub fn clear(&self) -> PyResult<()> {
        self.reader.clear()
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        self.reader.is_empty()
    }

    pub fn len(&self) -> PyResult<usize> {
        self.reader.len()
    }

    pub fn read(&self, py: Python) -> PyResult<PyMessageReaderIter> {
        self.reader.read(py)
    }

    pub fn __iter__(&self, py: Python) -> PyResult<PyMessageReaderIter> {
        self.read(py)
    }
}

/// Write a Python message to its App-local custom channel.
/// Called from external crates via `global_registry::write_python_message`.
fn write_custom_python_message(
    world: &mut bevy::ecs::world::World,
    _py: Python,
    msg: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let type_ptr = msg.get_type().as_type_ptr();
    let resolved = resolve_from_world(world, type_ptr)?;
    resolved
        .store
        .append(resolved.channel, Arc::new(msg.clone().unbind()))
        .map_err(store_error)?;
    Ok(())
}

/// Register the message write function in the global registry.
/// Called once at module init.
pub(crate) fn register_message_write_fn() {
    global_registry::register_message_write_fn(|world, py, msg| {
        write_custom_python_message(world, py, msg).map_err(|e| format!("{e}"))
    });
}

/// Register the run_system_once function in the global registry.
/// Called once at module init.
pub(crate) fn register_run_system_once_fn() {
    global_registry::register_run_system_once_fn(|world, py, func| {
        PyWorld::with_temporary(world, py, |py_world| py_world.run_system_once(func.clone()))
            .map_err(|e| format!("{e}"))
    });
}
