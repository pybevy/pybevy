//! PyO3 adapter state for the interpreter-neutral Python message store.

use std::{alloc::Layout, collections::HashMap, sync::Arc};

use bevy::{
    app::{App, First},
    ecs::{
        component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
        message::{MessageUpdateSystems, message_update_condition, message_update_system},
        prelude::{Mut, ResMut, Resource},
        schedule::IntoScheduleConfigs,
        world::{World, unsafe_world_cell::UnsafeWorldCell},
    },
};
use pybevy_ecs::shared::message_store::{
    MessageChannelId, MessageRegisterOutcome, MessageRegistryCore, MessageStore, MessageStoreError,
    MessageTypeKey,
};
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyType, PyTypeMethods},
};

use crate::ecs::message::PyMessage;

pub(crate) type PythonMessageValue = Arc<Py<PyAny>>;
pub(crate) type PythonMessageStore = MessageStore<PythonMessageValue>;

/// Strong backend class handles paired with the neutral identity registry.
#[derive(Default, Resource)]
pub(crate) struct PythonMessageClassTable {
    by_type: HashMap<MessageTypeKey, Arc<Py<PyType>>>,
    newest_by_channel: HashMap<MessageChannelId, Arc<Py<PyType>>>,
}

impl PythonMessageClassTable {
    fn insert(
        &mut self,
        type_key: MessageTypeKey,
        channel: MessageChannelId,
        class: Arc<Py<PyType>>,
    ) -> Vec<Arc<Py<PyType>>> {
        let mut replaced = Vec::new();
        if let Some(old) = self.by_type.insert(type_key, Arc::clone(&class)) {
            replaced.push(old);
        }
        if let Some(old) = self.newest_by_channel.insert(channel, class) {
            replaced.push(old);
        }
        replaced
    }

    pub(crate) fn exact(&self, type_key: MessageTypeKey) -> Option<Arc<Py<PyType>>> {
        self.by_type.get(&type_key).cloned()
    }

    fn remove_exact(&mut self, type_keys: &[MessageTypeKey]) -> Vec<Arc<Py<PyType>>> {
        type_keys
            .iter()
            .filter_map(|type_key| self.by_type.remove(type_key))
            .collect()
    }
}

/// Context-free state captured by a custom reader or writer wrapper.
#[derive(Clone)]
pub(crate) struct ResolvedPythonMessage {
    pub store: PythonMessageStore,
    pub channel: MessageChannelId,
    pub class: Arc<Py<PyType>>,
}

fn type_key(type_ptr: *const PyTypeObject) -> MessageTypeKey {
    MessageTypeKey::new(type_ptr as usize)
}

fn qualified_name(message: &Bound<'_, PyType>) -> PyResult<String> {
    let module = message.getattr("__module__")?.extract::<String>()?;
    let qualname = message.getattr("__qualname__")?.extract::<String>()?;
    Ok(format!("{module}.{qualname}"))
}

fn register_access_id(
    world: &mut World,
    channel: MessageChannelId,
    qualified_name: &str,
) -> ComponentId {
    let name = format!("PyMessageAccess<{qualified_name}>#{}", channel.get());
    // SAFETY: this is a zero-sized, Send + Sync, access-only component that is
    // never inserted. Its unit layout needs no drop function or relationship.
    let descriptor = unsafe {
        ComponentDescriptor::new_with_layout(
            name,
            StorageType::Table,
            Layout::new::<()>(),
            None,
            true,
            ComponentCloneBehavior::Default,
            None,
        )
    };
    world.register_component_with_descriptor(descriptor)
}

fn core_error(error: MessageStoreError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

/// Register one Python-defined message class in an existing World.
pub(crate) fn register_python_message(
    py: Python<'_>,
    world: &mut World,
    message: &Bound<'_, PyType>,
    generation: u32,
) -> PyResult<MessageRegisterOutcome> {
    if !message.is_subclass_of::<PyMessage>()? {
        return Err(PyTypeError::new_err("Expected a subclass of `Message`"));
    }

    if !world.contains_resource::<MessageRegistryCore>() {
        world.init_resource::<MessageRegistryCore>();
    }
    if !world.contains_resource::<PythonMessageStore>() {
        world.init_resource::<PythonMessageStore>();
    }
    if !world.contains_resource::<PythonMessageClassTable>() {
        world.init_resource::<PythonMessageClassTable>();
    }

    let key = type_key(message.as_type_ptr());
    let name = qualified_name(message)?;
    let outcome = world.resource_scope(|world, mut registry: Mut<MessageRegistryCore>| {
        registry.register(key, &name, generation, |channel| {
            register_access_id(world, channel, &name)
        })
    });
    let outcome = outcome.map_err(core_error)?;
    let channel = outcome.channel();

    let store = world.resource::<PythonMessageStore>().clone();
    if matches!(outcome, MessageRegisterOutcome::Registered(_)) {
        store.register_channel(channel);
    } else if matches!(outcome, MessageRegisterOutcome::Aliased(_)) {
        // A new class object must not receive retained instances of the class
        // it replaced, even during a partial reload.
        let retired = store.clear_channel(channel).map_err(core_error)?;
        drop(retired);
    }

    let class = Arc::new(message.as_unbound().clone_ref(py));
    let replaced = {
        let mut table = world.resource_mut::<PythonMessageClassTable>();
        table.insert(key, channel, class)
    };
    drop(replaced);
    Ok(outcome)
}

/// Resolve a parameter's exact class using only declared resource reads.
///
/// # Safety
/// The caller must have declared reads for the store, registry, and class table
/// and keep `world` valid for the duration of this function.
pub(crate) unsafe fn resolve_from_cell(
    world: UnsafeWorldCell<'_>,
    type_ptr: *const PyTypeObject,
) -> PyResult<ResolvedPythonMessage> {
    // SAFETY: upheld by the caller and restricted to the declared resources.
    let key = type_key(type_ptr);
    let channel = {
        let registry = unsafe { world.get_resource::<MessageRegistryCore>() }
            .ok_or_else(|| PyTypeError::new_err("Message registry is not initialized"))?;
        registry.channel_for_type(key).ok_or_else(|| {
            PyTypeError::new_err("Message type is not registered; call app.add_message(T) first")
        })?
    };

    // SAFETY: upheld by the caller and restricted to the declared resource.
    let store = unsafe { world.get_resource::<PythonMessageStore>() }
        .ok_or_else(|| PyTypeError::new_err("Python message store is not initialized"))?
        .clone();
    // SAFETY: upheld by the caller and restricted to the declared resource.
    let table = unsafe { world.get_resource::<PythonMessageClassTable>() }
        .ok_or_else(|| PyTypeError::new_err("Message class table is not initialized"))?;
    let class = table
        .exact(key)
        .ok_or_else(|| PyTypeError::new_err("Message class is not registered"))?;
    Ok(ResolvedPythonMessage {
        store,
        channel,
        class,
    })
}

pub(crate) fn resolve_from_world(
    world: &World,
    type_ptr: *const PyTypeObject,
) -> PyResult<ResolvedPythonMessage> {
    // SAFETY: an exclusive observer/external caller owns this live World.
    unsafe { resolve_from_cell(world.as_unsafe_world_cell_readonly(), type_ptr) }
}

/// Clear every custom channel while retaining channel identity and sequences.
pub(crate) fn clear_python_messages(world: &World) {
    let Some(store) = world.get_resource::<PythonMessageStore>().cloned() else {
        return;
    };
    let retired = store.clear_all();
    Python::attach(|_| drop(retired));
}

/// Prune stale exact class aliases and release their strong handles safely.
pub(crate) fn prune_python_message_aliases(world: &mut World, minimum_generation: u32) {
    let removed_keys = world
        .get_resource_mut::<MessageRegistryCore>()
        .map(|mut registry| registry.prune_aliases(minimum_generation))
        .unwrap_or_default();
    if removed_keys.is_empty() {
        return;
    }
    let retired_classes = world
        .get_resource_mut::<PythonMessageClassTable>()
        .map(|mut table| table.remove_exact(&removed_keys))
        .unwrap_or_default();
    Python::attach(|_| drop(retired_classes));
}

fn maintain_python_messages(store: ResMut<PythonMessageStore>) {
    let retired = store.advance();
    drop(store);
    Python::attach(|_| drop(retired));
}

/// Install App-local custom-message infrastructure exactly once.
pub(crate) fn install_python_message_store(app: &mut App) {
    app.init_resource::<MessageRegistryCore>()
        .init_resource::<PythonMessageStore>()
        .init_resource::<PythonMessageClassTable>()
        .add_systems(
            First,
            maintain_python_messages
                .in_set(MessageUpdateSystems)
                .before(message_update_system)
                .run_if(message_update_condition),
        );
}
