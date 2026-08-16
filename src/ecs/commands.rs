use std::{
    any::TypeId,
    sync::{Arc, Mutex},
};

use bevy::ecs::{
    component::ComponentId,
    entity::Entity,
    hierarchy::ChildOf,
    ptr::OwningPtr,
    resource::IsResource,
    system::Commands,
    world::{CommandQueue, World},
};
use pybevy_core::{
    ComponentBridge, LogicalTypeId, LogicalTypeMap, PyLogicalComponentParam,
    custom_resource::validate_hierarchy_link,
    ensure_no_live_asset_access,
    public_error::{
        IS_RESOURCE_COMPONENT_REMOVE, RESOURCE_COMPONENT_INSERT, RESOURCE_COMPONENT_REMOVE,
        RESOURCE_COMPONENT_SPAWN, RESOURCE_ENTITY_DESPAWN,
    },
    registry::global_registry,
};
use pybevy_ecs::shared::{
    bundle_validation::first_duplicate_indices,
    parity_trace::{CanonValue, ParityOpKind, ParityRunHandle, PendingParityOp},
    system_runtime::ErrorPolicy,
};
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyTuple, PyType},
};

use super::{
    PyEntity,
    component_type::{
        PreparedCustomComponentRegistration, PyComponentType, register_prepared_custom_component,
    },
    entity_commands::PyEntityCommands,
    helpers::validity_guard::ValidityFlag,
    resource::hierarchy_contains_resource_entity,
    resource_type::PyResourceType,
    world::PyWorld,
};
use crate::ecs::{
    batch_spawn::{SpawnBatchCommand, prepare_iter_batch},
    component_layout::{
        ComponentLayout, ComponentLayoutExt, ComponentStorageType, serialize_to_wrapper,
    },
    component_wrapper::*,
    dynamic_system::{BufferedSystemError, SystemErrorBuffer, lock_or_recover},
    observer::{PyEvent, PyOn},
    observer_registry::ObserverRegistry,
    parity_trace::{canonicalize_payload, canonicalize_payload_without_root_field},
};

pub(crate) fn trigger_event_helper(
    commands: &PyCommands,
    py: Python,
    event: Bound<'_, PyAny>,
    target_entity: Option<Entity>,
) -> PyResult<()> {
    commands.check_valid()?;

    if !event.get_type().is_subclass_of::<PyEvent>()? {
        return Err(PyRuntimeError::new_err(
            "trigger() requires an Event subclass instance",
        ));
    }

    let trace_operation =
        commands.prepare_trace_op(ParityOpKind::ObserverTrigger, &event, target_entity)?;
    let event_clone = event.clone().unbind();

    if commands.is_world {
        let world = commands.world_mut()?;
        ensure_no_live_asset_access(world, "commands.trigger()")
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let observers = world
            .get_resource::<ObserverRegistry>()
            .map(|registry| registry.snapshot_user_event(&event, target_entity))
            .unwrap_or_default();
        for observer_entry in observers {
            if !ObserverRegistry::matches_user_filter(&observer_entry, world, target_entity) {
                continue;
            }

            let on_param = Py::new(
                py,
                PyOn {
                    event_data: event_clone.clone_ref(py),
                    entity: target_entity,
                },
            )?;
            ObserverRegistry::invoke(
                &observer_entry,
                world,
                &on_param,
                target_entity,
                ErrorPolicy::PropagateToCaller,
            )?;
        }
    } else {
        commands.execute_or_queue(move |world: &mut World| {
            Python::attach(|py| {
                let event_bound = event_clone.bind(py);
                let observers = world
                    .get_resource::<ObserverRegistry>()
                    .map(|registry| registry.snapshot_user_event(event_bound, target_entity))
                    .unwrap_or_default();
                for observer_entry in observers {
                    if !ObserverRegistry::matches_user_filter(&observer_entry, world, target_entity)
                    {
                        continue;
                    }

                    if let Ok(on_param) = Py::new(
                        py,
                        PyOn {
                            event_data: event_clone.clone_ref(py),
                            entity: target_entity,
                        },
                    ) {
                        let _ = ObserverRegistry::invoke(
                            &observer_entry,
                            world,
                            &on_param,
                            target_entity,
                            ErrorPolicy::ReportAndContinue,
                        );
                    }
                }
            });
        })?;
    }

    commands.record_prepared_trace_op(trace_operation);
    Ok(())
}

pub(crate) fn reject_resource_spawn_components(
    py: Python,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    for component in components.iter() {
        if matches!(
            PyComponentType::try_from((&component.get_type(), py))?,
            PyComponentType::Resource(_)
        ) {
            return Err(PyTypeError::new_err(RESOURCE_COMPONENT_SPAWN));
        }
    }
    Ok(())
}

pub(crate) fn normalize_spawn_components<'py>(
    components: &Bound<'py, PyTuple>,
) -> PyResult<Bound<'py, PyTuple>> {
    if components.len() == 1 {
        let first = components.get_item(0)?;
        if let Ok(bundle) = first.cast_exact::<PyTuple>() {
            return Ok(bundle.clone());
        }
    }
    Ok(components.clone())
}

pub(crate) fn validate_component_bundle(
    py: Python,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    let mut keys = Vec::with_capacity(components.len());
    let mut names = Vec::with_capacity(components.len());

    for component in components.iter() {
        let component_type = PyComponentType::try_from((&component.get_type(), py))?;
        keys.push(component_type.validation_identity());
        names.push(component.get_type().name()?.to_string());
    }

    if let Some((first, duplicate)) = first_duplicate_indices(keys) {
        return Err(PyValueError::new_err(format!(
            "component bundle contains duplicate '{}'; it was already supplied as '{}'",
            names[duplicate], names[first]
        )));
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) struct CommandErrorSink {
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
    retain_exception: bool,
}

impl CommandErrorSink {
    pub(crate) fn new(
        error_state: Arc<Mutex<Vec<PyErr>>>,
        error_buffer: SystemErrorBuffer,
        retain_exception: bool,
    ) -> Self {
        Self {
            error_state,
            error_buffer,
            retain_exception,
        }
    }

    fn record(&self, error: PyErr) {
        Python::attach(|py| {
            let message = error.to_string();
            let traceback = error.traceback(py).map(|traceback| {
                traceback
                    .format()
                    .unwrap_or_else(|_| "(traceback format failed)".into())
            });
            *lock_or_recover(&self.error_buffer) = Some(BufferedSystemError {
                error: message,
                traceback,
            });
            if self.retain_exception {
                lock_or_recover(&self.error_state).push(error);
            } else {
                drop(error);
            }
        });
    }
}

/// Wrapper around Bevy's Commands system parameter.
/// Commands queue operations to be applied to the World after the system completes.
///
/// Safety: This type is Send because:
/// 1. Raw pointers are just addresses (Send if we guarantee validity)
/// 2. ValidityFlag is Arc<AtomicBool> which is Send + Sync
/// 3. Runtime validity checking prevents use after the system completes
/// 4. The optional PyWorld reference is also Send (Py<T> is Send if T is Send)
#[pyclass(name = "Commands")]
pub struct PyCommands {
    commands_ptr: *mut (),
    is_world: bool, // Flag to indicate if this wraps a World instead of Commands
    is_queue: bool,
    // Keep the PyWorld alive if we're wrapping a World
    _world_ref: Option<Py<PyWorld>>,
    // Runtime validity check - prevents use after system execution
    validity: ValidityFlag,
    error_sink: Option<CommandErrorSink>,
    parity_trace: Option<ParityRunHandle>,
}

// SAFETY: PyCommands is Send because:
// - The raw pointer is protected by the ValidityFlag (Arc<AtomicBool>)
// - ValidityFlag::check() ensures the pointer is only dereferenced when valid
// - The validity flag is set to false when the system execution completes
// - Py<PyWorld> is Send when PyWorld is Send
unsafe impl Send for PyCommands {}

// SAFETY: PyCommands is Sync because:
// - Access to the underlying Commands/World is controlled by validity checking
// - The ValidityFlag uses atomic operations for thread-safe access
// - We only allow access when the validity flag is true (during system execution)
unsafe impl Sync for PyCommands {}

impl PyCommands {
    /// Create a new PyCommands wrapper around a mutable Commands reference.
    ///
    /// # Safety
    /// The commands pointer must be valid for the lifetime of this PyCommands instance.
    /// This should only be created within the system's run_unsafe and dropped before returning.
    pub(crate) unsafe fn new(
        commands: &mut Commands,
        validity: ValidityFlag,
        error_sink: CommandErrorSink,
        parity_trace: Option<ParityRunHandle>,
    ) -> Self {
        Self {
            commands_ptr: commands as *mut Commands as *mut (),
            is_world: false,
            is_queue: false,
            _world_ref: None,
            validity,
            error_sink: Some(error_sink),
            parity_trace,
        }
    }

    /// Create a PyCommands that wraps a World pointer
    ///
    /// # Safety
    /// The world pointer must be valid for the lifetime of this PyCommands instance.
    pub(crate) unsafe fn from_world(
        world_ptr: *mut World,
        world_ref: Py<PyWorld>,
        validity: ValidityFlag,
    ) -> Self {
        Self {
            commands_ptr: world_ptr as *mut (),
            is_world: true,
            is_queue: false,
            _world_ref: Some(world_ref),
            validity,
            error_sink: None,
            parity_trace: None,
        }
    }

    /// Create a temporary PyCommands that wraps a World pointer without owning a PyWorld reference
    /// Used internally when we're already within a PyWorld method
    ///
    /// # Safety
    /// The world pointer must be valid for the lifetime of this PyCommands instance.
    pub(crate) unsafe fn from_world_temporary(
        world_ptr: *mut World,
        validity: ValidityFlag,
    ) -> Self {
        Self {
            commands_ptr: world_ptr as *mut (),
            is_world: true,
            is_queue: false,
            _world_ref: None,
            validity,
            error_sink: None,
            parity_trace: None,
        }
    }

    /// Create a temporary wrapper that records only owned structural commands.
    ///
    /// # Safety
    /// `queue` must outlive the wrapper and must not be accessed concurrently.
    unsafe fn from_queue_temporary(
        queue: &mut CommandQueue,
        validity: ValidityFlag,
        error_sink: Option<CommandErrorSink>,
    ) -> Self {
        Self {
            commands_ptr: queue as *mut CommandQueue as *mut (),
            is_world: false,
            is_queue: true,
            _world_ref: None,
            validity,
            error_sink,
            parity_trace: None,
        }
    }

    /// Check if this Commands instance is still valid for use
    fn check_valid(&self) -> PyResult<()> {
        Ok(self.validity.check()?)
    }

    /// Get a clone of the validity flag for sharing with child structures
    pub(crate) fn validity(&self) -> ValidityFlag {
        self.validity.clone()
    }

    pub(crate) fn error_sink(&self) -> Option<CommandErrorSink> {
        self.error_sink.clone()
    }

    // validity-checked raw pointer access, see docs/safety.md
    #[allow(clippy::mut_from_ref)]
    fn commands_mut(&self) -> PyResult<&mut Commands<'_, '_>> {
        self.validity.check()?;
        if self.is_world || self.is_queue {
            return Err(PyRuntimeError::new_err(
                "Cannot get Commands from World-backed PyCommands",
            ));
        }
        Ok(unsafe { &mut *(self.commands_ptr as *mut Commands) })
    }

    // validity-checked raw pointer access, see docs/safety.md
    #[allow(clippy::mut_from_ref)]
    fn world_mut(&self) -> PyResult<&mut World> {
        self.validity.check()?;
        if !self.is_world {
            return Err(PyRuntimeError::new_err(
                "Cannot get World from Commands-backed PyCommands",
            ));
        }
        Ok(unsafe { &mut *(self.commands_ptr as *mut World) })
    }

    pub(crate) fn check_native_asset_access(&self, operation: &str) -> PyResult<()> {
        if !self.is_world {
            return Ok(());
        }
        let world = self.world_mut()?;
        ensure_no_live_asset_access(world, operation)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    /// Get world access if this is world-backed, otherwise return None
    // validity-checked raw pointer access, see docs/safety.md
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn try_world_mut(&self) -> PyResult<Option<&mut World>> {
        self.validity.check()?;
        if self.is_world {
            Ok(Some(unsafe { &mut *(self.commands_ptr as *mut World) }))
        } else {
            Ok(None)
        }
    }

    /// Execute an operation immediately on World or queue it as a Command.
    /// For operations that don't need a return value.
    pub(crate) fn execute_or_queue<F>(&self, operation: F) -> PyResult<()>
    where
        F: FnOnce(&mut World) + Send + 'static,
    {
        if self.is_world {
            let world = self.world_mut()?;
            operation(world);
        } else if self.is_queue {
            self.validity.check()?;
            // SAFETY: `from_queue_temporary` provides the only live mutable
            // access and the validity fence covers this append.
            unsafe { &mut *(self.commands_ptr as *mut CommandQueue) }.push(operation);
        } else {
            let commands = self.commands_mut()?;
            commands.queue(operation);
        }
        Ok(())
    }

    /// Execute an operation that returns a value.
    /// Requires separate closures for world and commands cases since they may have different logic.
    fn execute_returning<T, FW, FC>(&self, world_op: FW, commands_op: FC) -> PyResult<T>
    where
        FW: FnOnce(&mut World) -> T,
        FC: FnOnce(&mut Commands) -> T,
    {
        if self.is_world {
            Ok(world_op(self.world_mut()?))
        } else if self.is_queue {
            Err(PyRuntimeError::new_err(
                "This temporary command queue cannot reserve or return entities",
            ))
        } else {
            Ok(commands_op(self.commands_mut()?))
        }
    }

    fn trace_spawn(&self, entity: Entity) {
        if let Some(trace) = &self.parity_trace {
            trace.record_spawn(entity, &CanonValue::None);
        }
    }

    fn trace_target_op(&self, kind: ParityOpKind, entity: Entity) {
        if let Some(trace) = &self.parity_trace {
            trace.record_op(PendingParityOp {
                kind,
                type_name: None,
                payload_digest: CanonValue::None.digest(),
                target: Some(entity),
            });
        }
    }

    fn prepare_trace_op(
        &self,
        kind: ParityOpKind,
        value: &Bound<'_, PyAny>,
        target: Option<Entity>,
    ) -> PyResult<Option<PendingParityOp>> {
        if self.parity_trace.is_none() {
            return Ok(None);
        }
        let payload = if kind == ParityOpKind::ObserverTrigger && target.is_some() {
            canonicalize_payload_without_root_field(value, "entity")?
        } else {
            canonicalize_payload(value)?
        };
        Ok(Some(PendingParityOp {
            kind,
            type_name: Some(value.get_type().name()?.to_string()),
            payload_digest: payload.digest(),
            target,
        }))
    }

    fn record_prepared_trace_op(&self, operation: Option<PendingParityOp>) {
        if let (Some(trace), Some(operation)) = (&self.parity_trace, operation) {
            trace.record_op(operation);
        }
    }

    fn prepare_uniform_batch_trace_payloads(
        &self,
        components: &Bound<'_, PyTuple>,
    ) -> PyResult<Vec<(String, String)>> {
        if self.parity_trace.is_none() {
            return Ok(Vec::new());
        }
        components
            .iter()
            .map(|component| {
                if global_registry::get_batch_bridge_by_py_type(
                    component.get_type().as_type_ptr(),
                )
                .is_some()
                {
                    return Err(PyTypeError::new_err(
                        "parity trace payload is unhashable: columnar spawn_batch components require a per-row canonicalization bridge",
                    ));
                }
                Ok((
                    component.get_type().name()?.to_string(),
                    canonicalize_payload(&component)?.digest(),
                ))
            })
            .collect()
    }

    fn record_batch_inserts(&self, entities: &[Entity], payloads: &[(String, String)]) {
        let Some(trace) = &self.parity_trace else {
            return;
        };
        for &entity in entities {
            for (type_name, payload_digest) in payloads {
                trace.record_op(PendingParityOp {
                    kind: ParityOpKind::Insert,
                    type_name: Some(type_name.clone()),
                    payload_digest: payload_digest.clone(),
                    target: Some(entity),
                });
            }
        }
    }
}

/// Report a failure from a deferred command through the system error pipeline.
pub(crate) fn report_deferred_error(sink: &Option<CommandErrorSink>, context: &str, error: PyErr) {
    match sink {
        Some(sink) => sink.record(error),
        None => eprintln!("Error: {context}: {error:?}"),
    }
}

fn entity_not_found_error(entity_id: Entity) -> PyErr {
    PyValueError::new_err(format!("Entity {entity_id:?} does not exist in the world"))
}

fn entity_exists(world: &World, entity_id: Entity) -> bool {
    world.get_entity(entity_id).is_ok()
}

fn ensure_entity_exists(world: &World, entity_id: Entity) -> PyResult<()> {
    if entity_exists(world, entity_id) {
        Ok(())
    } else {
        Err(entity_not_found_error(entity_id))
    }
}

fn ensure_entities_exist(world: &World, entity_ids: &[Entity]) -> PyResult<()> {
    for entity_id in entity_ids {
        ensure_entity_exists(world, *entity_id)?;
    }
    Ok(())
}

pub(crate) fn component_logical_type(
    component: &Bound<'_, PyAny>,
) -> PyResult<Option<Option<LogicalTypeId>>> {
    let Ok(value) = component.getattr("_logical_type_id") else {
        return Ok(None);
    };
    Ok(Some(
        value.extract::<Option<u64>>()?.map(LogicalTypeId::new),
    ))
}

pub(crate) fn update_entity_logical_type(
    world: &mut World,
    entity: Entity,
    native_type: std::any::TypeId,
    logical_type: Option<LogicalTypeId>,
) {
    match logical_type {
        Some(logical_type) => {
            if let Some(mut map) = world.get_mut::<LogicalTypeMap>(entity) {
                map.insert(native_type, logical_type);
            } else {
                let mut map = LogicalTypeMap::default();
                map.insert(native_type, logical_type);
                world.entity_mut(entity).insert(map);
            }
        }
        None => {
            let remove_map = world
                .get_mut::<LogicalTypeMap>(entity)
                .is_some_and(|mut map| {
                    map.remove(native_type);
                    map.is_empty()
                });
            if remove_map {
                world.entity_mut(entity).remove::<LogicalTypeMap>();
            }
        }
    }
}

pub(crate) fn entity_logical_type_matches(
    world: &World,
    entity: Entity,
    component_type: PyComponentType,
    logical_type: LogicalTypeId,
) -> bool {
    component_type.type_id().is_some_and(|native_type| {
        world
            .get::<LogicalTypeMap>(entity)
            .is_some_and(|map| map.matches(native_type, logical_type))
    })
}

fn insert_custom_wrapper_bytes(
    world: &mut World,
    entity_id: Entity,
    component_id: ComponentId,
    wrapper_size: WrapperSize,
    bytes: &[u8],
) {
    let mut entity = world.entity_mut(entity_id);
    insert_wrapper_bytes(&mut entity, component_id, wrapper_size, bytes)
        .expect("prepared custom wrapper registration must match its serialized bytes");
}

enum PreparedCustomComponentValue {
    Wrapper {
        bytes: Vec<u8>,
        wrapper_size: WrapperSize,
        retained_type: Py<PyType>,
    },
    PyObject(Py<PyAny>),
}

fn prepare_custom_component(
    component: &Bound<'_, PyAny>,
) -> PyResult<(
    PreparedCustomComponentRegistration,
    PreparedCustomComponentValue,
)> {
    let component_type = component.get_type();
    let registration = PreparedCustomComponentRegistration::from_python_class(&component_type)?;
    let value = match registration.storage_type() {
        ComponentStorageType::Wrapper(wrapper_size) => {
            let layout = ComponentLayout::from_annotations(&component_type)?;
            debug_assert_eq!(layout.wrapper_size, wrapper_size);
            let bytes = serialize_to_wrapper(component, &layout)?;
            debug_assert_eq!(bytes.len(), wrapper_size.size_bytes());
            PreparedCustomComponentValue::Wrapper {
                bytes,
                wrapper_size,
                retained_type: component_type.unbind(),
            }
        }
        ComponentStorageType::PyObject => {
            PreparedCustomComponentValue::PyObject(component.clone().unbind())
        }
    };
    Ok((registration, value))
}

/// Helper function to insert components to an entity
pub(crate) fn insert_components_to_entity_helper(
    commands: &PyCommands,
    py: Python,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    validate_component_bundle(py, components)?;

    let trace_operations = if commands.parity_trace.is_some() {
        components
            .iter()
            .map(|component| {
                commands.prepare_trace_op(ParityOpKind::Insert, &component, Some(entity_id))
            })
            .collect::<PyResult<Vec<_>>>()?
    } else {
        Vec::new()
    };

    // Collect component types for lifecycle events
    let mut component_types = Vec::new();
    for component in components.iter() {
        let component_type = component.get_type();
        if let Ok(comp_type) = PyComponentType::try_from((&component_type, py)) {
            component_types.push(comp_type);
        }
    }

    if component_types.is_empty() {
        insert_components_to_entity(commands, py, entity_id, components)?;
        for operation in trace_operations {
            commands.record_prepared_trace_op(operation);
        }
        return Ok(());
    }

    if commands.is_world {
        commands.check_native_asset_access("entity.insert()")?;
        let validity = commands.validity.clone();
        let world = commands.world_mut()?;
        ensure_entity_exists(world, entity_id)?;
        let mut insertion_error = None;
        crate::ecs::lifecycle_mutation::insert_many_with(
            world,
            entity_id,
            &component_types,
            |world| {
                // SAFETY: this temporary wrapper is a reborrow of the
                // adapter's exclusive World for the structural commit only.
                // It cannot escape this closure and is never used alongside
                // the adapter's `&mut World`.
                let temporary =
                    unsafe { PyCommands::from_world_temporary(world as *mut World, validity) };
                match insert_components_to_entity(&temporary, py, entity_id, components) {
                    Ok(()) => true,
                    Err(error) => {
                        insertion_error = Some(error);
                        false
                    }
                }
            },
        );
        if let Some(error) = insertion_error {
            return Err(error);
        }
    } else {
        // Prepare every fallible/borrowed Python conversion now, while the
        // scheduled system's validity window is still active. Only owned Rust
        // commands cross into the later lifecycle application closure.
        let mut structural_queue = CommandQueue::default();
        let temporary = unsafe {
            PyCommands::from_queue_temporary(
                &mut structural_queue,
                commands.validity.clone(),
                commands.error_sink.clone(),
            )
        };
        insert_components_to_entity(&temporary, py, entity_id, components)?;
        commands.execute_or_queue(move |world| {
            crate::ecs::lifecycle_mutation::insert_many_with(
                world,
                entity_id,
                &component_types,
                |world| {
                    structural_queue.apply(world);
                    true
                },
            );
        })?;
    }

    for operation in trace_operations {
        commands.record_prepared_trace_op(operation);
    }

    Ok(())
}

/// Internal helper function to insert components to an entity
fn insert_components_to_entity(
    commands: &PyCommands,
    py: Python,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    if commands.is_world {
        let world = commands.world_mut()?;
        ensure_entity_exists(world, entity_id)?;
    }

    for component in components.iter() {
        // Determine component type
        let component_type = PyComponentType::try_from((&component.get_type(), py))?;

        // Insert the component based on its type
        match component_type {
            // Children, GlobalTransform use dynamic dispatch from pybevy_core - bridges return appropriate errors
            // Gamepad, AudioSink, SpatialAudioSink now handled via bridge (no_insert returns error)
            PyComponentType::Dynamic(type_ptr) => {
                // Dynamic component - use bridge for insertion
                // Get the bridge for this type
                let bridge = global_registry::get_bridge_by_py_type(type_ptr).ok_or_else(|| {
                    PyRuntimeError::new_err("Dynamic component type not registered")
                })?;
                let logical_type = component_logical_type(&component)?;
                let native_type = bridge.bevy_type_id();

                if commands.is_world {
                    // Direct world access - insert immediately via bridge
                    let world = commands.world_mut()?;
                    validate_relationship_component(world, entity_id, &component, bridge.as_ref())?;
                    bridge.insert(world, entity_id, &component)?;
                    if let Some(logical_type) = logical_type {
                        update_entity_logical_type(world, entity_id, native_type, logical_type);
                    }
                } else {
                    // Commands - need to queue the operation
                    // Clone data needed for the deferred operation
                    let py_obj = component.clone().unbind();
                    let bridge_name = bridge.name();
                    let error_sink = commands.error_sink.clone();

                    commands.execute_or_queue(move |world: &mut World| {
                        if !entity_exists(world, entity_id) {
                            return;
                        }

                        // Re-acquire GIL and re-bind the component
                        Python::attach(|py| {
                            let component_bound = py_obj.bind(py);
                            let type_obj = component_bound.get_type();
                            let type_ptr = type_obj.as_type_ptr();

                            // Get the bridge again (it's registered globally)
                            if let Some(bridge) = global_registry::get_bridge_by_py_type(type_ptr) {
                                if let Err(error) = validate_relationship_component(
                                    world,
                                    entity_id,
                                    component_bound,
                                    bridge.as_ref(),
                                ) {
                                    match &error_sink {
                                        Some(sink) => sink.record(error),
                                        None => eprintln!(
                                            "Failed to validate dynamic component '{}': relationship is invalid",
                                            bridge_name
                                        ),
                                    }
                                    return;
                                }
                                match bridge.insert(world, entity_id, component_bound) {
                                    Ok(()) => {
                                        if let Some(logical_type) = logical_type {
                                            update_entity_logical_type(
                                                world,
                                                entity_id,
                                                native_type,
                                                logical_type,
                                            );
                                        }
                                    }
                                    Err(error) => report_deferred_error(
                                        &error_sink,
                                        &format!(
                                            "Failed to insert component '{bridge_name}' via Commands"
                                        ),
                                        error,
                                    ),
                                }
                            }
                        });
                    })?;
                }
            }
            PyComponentType::Resource(_) => {
                return Err(PyTypeError::new_err(RESOURCE_COMPONENT_INSERT));
            }
            PyComponentType::Custom(_) => {
                let (registration, prepared_value) = prepare_custom_component(&component)?;

                if commands.is_world {
                    let world = commands.world_mut()?;
                    let component_id = register_prepared_custom_component(world, &registration);
                    match prepared_value {
                        PreparedCustomComponentValue::Wrapper {
                            bytes,
                            wrapper_size,
                            ..
                        } => insert_custom_wrapper_bytes(
                            world,
                            entity_id,
                            component_id,
                            wrapper_size,
                            &bytes,
                        ),
                        PreparedCustomComponentValue::PyObject(value) => {
                            OwningPtr::make(value, |ptr| {
                                let mut entity = world.entity_mut(entity_id);
                                // SAFETY: the prepared registration selected
                                // Py<PyAny> storage for this exact component ID.
                                unsafe {
                                    entity.insert_by_id(component_id, ptr);
                                }
                            });
                        }
                    }
                } else {
                    commands.execute_or_queue(move |world: &mut World| {
                        Python::attach(move |_py| {
                            if !entity_exists(world, entity_id) {
                                return;
                            }

                            let component_id =
                                register_prepared_custom_component(world, &registration);
                            match prepared_value {
                                PreparedCustomComponentValue::Wrapper {
                                    bytes,
                                    wrapper_size,
                                    retained_type: _retained_type,
                                } => insert_custom_wrapper_bytes(
                                    world,
                                    entity_id,
                                    component_id,
                                    wrapper_size,
                                    &bytes,
                                ),
                                PreparedCustomComponentValue::PyObject(value) => {
                                    OwningPtr::make(value, |ptr| {
                                        let mut entity = world.entity_mut(entity_id);
                                        // SAFETY: the prepared registration selected
                                        // Py<PyAny> storage for this exact component ID.
                                        unsafe {
                                            entity.insert_by_id(component_id, ptr);
                                        }
                                    });
                                }
                            }
                        });
                    })?;
                }
            }
        }
    }

    Ok(())
}

fn validate_relationship_component(
    world: &World,
    child: Entity,
    component: &Bound<'_, PyAny>,
    bridge: &dyn ComponentBridge,
) -> PyResult<()> {
    let Some(field) = bridge.relationship_field() else {
        return Ok(());
    };
    let parent = component.getattr(field)?.extract::<PyEntity>()?.0;
    validate_hierarchy_link(world, child, parent)
        .map_err(|error| PyTypeError::new_err(error.to_string()))
}

/// Helper function to add a child to an entity
pub(crate) fn add_child_helper(
    commands: &PyCommands,
    parent_id: Entity,
    child_id: Entity,
) -> PyResult<()> {
    if commands.is_world {
        let world = commands.world_mut()?;
        ensure_entities_exist(world, &[parent_id, child_id])?;
        validate_hierarchy_link(world, child_id, parent_id)
            .map_err(|error| PyTypeError::new_err(error.to_string()))?;
        ensure_no_live_asset_access(world, "entity.add_child()")
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        world.entity_mut(parent_id).add_child(child_id);
    } else {
        let error_sink = commands.error_sink.clone();
        commands.execute_or_queue(move |world| {
            if entity_exists(world, parent_id) && entity_exists(world, child_id) {
                if let Err(error) = validate_hierarchy_link(world, child_id, parent_id) {
                    let error = PyTypeError::new_err(error.to_string());
                    match &error_sink {
                        Some(sink) => sink.record(error),
                        None => eprintln!("Error: Failed to add child via Commands: {error:?}"),
                    }
                    return;
                }
                world.entity_mut(parent_id).add_child(child_id);
            }
        })?;
    }
    Ok(())
}

/// Helper function to remove children from an entity
pub(crate) fn remove_children_helper(
    commands: &PyCommands,
    parent_id: Entity,
    child_ids: &[Entity],
) -> PyResult<()> {
    if commands.is_world {
        let world = commands.world_mut()?;
        ensure_entity_exists(world, parent_id)?;
        ensure_entities_exist(world, child_ids)?;
        ensure_no_live_asset_access(world, "entity.remove_children()")
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        world.entity_mut(parent_id).detach_children(child_ids);
    } else {
        let child_ids = child_ids.to_vec();
        commands.execute_or_queue(move |world| {
            if !entity_exists(world, parent_id) {
                return;
            }

            let existing_children = child_ids
                .iter()
                .copied()
                .filter(|child_id| entity_exists(world, *child_id))
                .collect::<Vec<_>>();
            if !existing_children.is_empty() {
                world
                    .entity_mut(parent_id)
                    .detach_children(&existing_children);
            }
        })?;
    }
    Ok(())
}

/// Helper function to clear all children from an entity
pub(crate) fn clear_children_helper(commands: &PyCommands, parent_id: Entity) -> PyResult<()> {
    if commands.is_world {
        let world = commands.world_mut()?;
        ensure_entity_exists(world, parent_id)?;
        ensure_no_live_asset_access(world, "entity.clear_children()")
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        world.entity_mut(parent_id).detach_all_children();
    } else {
        commands.execute_or_queue(move |world| {
            if entity_exists(world, parent_id) {
                world.entity_mut(parent_id).detach_all_children();
            }
        })?;
    }
    Ok(())
}

/// Helper function to set the parent of an entity
pub(crate) fn set_parent_helper(
    commands: &PyCommands,
    child_id: Entity,
    parent_id: Entity,
) -> PyResult<()> {
    if commands.is_world {
        let world = commands.world_mut()?;
        ensure_entities_exist(world, &[child_id, parent_id])?;
        validate_hierarchy_link(world, child_id, parent_id)
            .map_err(|error| PyTypeError::new_err(error.to_string()))?;
        ensure_no_live_asset_access(world, "entity.set_parent()")
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        world.entity_mut(child_id).insert(ChildOf(parent_id));
    } else {
        let error_sink = commands.error_sink.clone();
        commands.execute_or_queue(move |world| {
            if entity_exists(world, child_id) && entity_exists(world, parent_id) {
                if let Err(error) = validate_hierarchy_link(world, child_id, parent_id) {
                    let error = PyTypeError::new_err(error.to_string());
                    match &error_sink {
                        Some(sink) => sink.record(error),
                        None => eprintln!("Error: Failed to set parent via Commands: {error:?}"),
                    }
                    return;
                }
                world.entity_mut(child_id).insert(ChildOf(parent_id));
            }
        })?;
    }
    Ok(())
}

/// Helper function to remove parent relationship from an entity
pub(crate) fn remove_parent_helper(commands: &PyCommands, child_id: Entity) -> PyResult<()> {
    if commands.is_world {
        let world = commands.world_mut()?;
        ensure_entity_exists(world, child_id)?;
        ensure_no_live_asset_access(world, "entity.remove_parent()")
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        world.entity_mut(child_id).remove::<ChildOf>();
    } else {
        commands.execute_or_queue(move |world| {
            if entity_exists(world, child_id) {
                world.entity_mut(child_id).remove::<ChildOf>();
            }
        })?;
    }
    Ok(())
}

/// Helper function to remove components from an entity
pub(crate) fn remove_components_from_entity_helper(
    commands: &PyCommands,
    py: Python,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    // Collect component types for lifecycle events
    let mut component_types = Vec::new();
    for component in components.iter() {
        if let Ok(logical) = component.extract::<PyRef<'_, PyLogicalComponentParam>>() {
            component_types.push((
                PyComponentType::Dynamic(logical.component_type_ptr()),
                Some(logical.logical_type_id()),
            ));
            continue;
        }
        let component_type_obj = component.cast::<PyType>().map_err(|_| {
            PyTypeError::new_err(
                "remove() expects component types (classes), not instances. Use Foo instead of Foo()",
            )
        })?;
        component_types.push((PyComponentType::try_from((component_type_obj, py))?, None));
    }

    for (component_type, logical_type) in component_types {
        if matches!(component_type, PyComponentType::Resource(_)) {
            return Err(PyTypeError::new_err(RESOURCE_COMPONENT_REMOVE));
        }
        if let PyComponentType::Dynamic(type_ptr) = component_type
            && global_registry::get_bridge_by_py_type(type_ptr)
                .is_some_and(|bridge| bridge.name() == "Children")
        {
            return Err(PyTypeError::new_err(
                "Cannot remove Children component - it is auto-managed by Bevy. Remove ChildOf components instead.",
            ));
        }
        if let PyComponentType::Dynamic(type_ptr) = component_type
            && global_registry::get_bridge_by_py_type(type_ptr)
                .is_some_and(|bridge| bridge.bevy_type_id() == TypeId::of::<IsResource>())
        {
            return Err(PyTypeError::new_err(IS_RESOURCE_COMPONENT_REMOVE));
        }
        if commands.is_world {
            let world = commands.world_mut()?;
            ensure_entity_exists(world, entity_id)?;
            if let Some(logical_type) = logical_type
                && !entity_logical_type_matches(world, entity_id, component_type, logical_type)
            {
                continue;
            }
            ensure_no_live_asset_access(world, "entity.remove()")
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            crate::ecs::lifecycle_mutation::remove(world, entity_id, component_type);
            if let Some(native_type) = component_type.type_id() {
                update_entity_logical_type(world, entity_id, native_type, None);
            }
        } else {
            commands.execute_or_queue(move |world| {
                if let Some(logical_type) = logical_type
                    && !entity_logical_type_matches(world, entity_id, component_type, logical_type)
                {
                    return;
                }
                crate::ecs::lifecycle_mutation::remove(world, entity_id, component_type);
                if let Some(native_type) = component_type.type_id() {
                    update_entity_logical_type(world, entity_id, native_type, None);
                }
            })?;
        }
    }

    Ok(())
}

#[pymethods]
impl PyCommands {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self._world_ref)
    }

    pub fn spawn_empty(&self, _py: Python<'_>) -> PyResult<PyEntityCommands> {
        self.check_valid()?;
        self.check_native_asset_access("commands.spawn_empty()")?;

        let entity = self.execute_returning(
            |world| world.spawn_empty().id(),
            |commands| commands.spawn_empty().id(),
        )?;
        self.trace_spawn(entity);

        Ok(PyEntityCommands::with_commands(entity, self))
    }

    #[pyo3(signature = (*components))]
    pub fn spawn(&self, py: Python, components: &Bound<'_, PyTuple>) -> PyResult<PyEntityCommands> {
        self.check_valid()?;
        let components_to_insert = normalize_spawn_components(components)?;

        reject_resource_spawn_components(py, &components_to_insert)?;
        validate_component_bundle(py, &components_to_insert)?;
        self.check_native_asset_access("commands.spawn()")?;

        let entity_id = self.execute_returning(
            |world| world.spawn_empty().id(),
            |commands| commands.spawn_empty().id(),
        )?;
        self.trace_spawn(entity_id);

        insert_components_to_entity_helper(self, py, entity_id, &components_to_insert)?;

        Ok(PyEntityCommands::with_commands(entity_id, self))
    }

    #[pyo3(signature = (*components, count=None))]
    pub fn spawn_batch(
        &self,
        py: Python,
        components: &Bound<'_, PyTuple>,
        count: Option<usize>,
    ) -> PyResult<Py<PyAny>> {
        self.check_valid()?;

        // A single list or iterator is treated as component bundles.
        if components.len() == 1 && count.is_none() {
            let first = components.get_item(0)?;
            if first.is_instance_of::<pyo3::types::PyList>() || first.hasattr("__next__")? {
                self.spawn_batch_iter(py, &first)?;
                return Ok(py.None());
            }
        }

        let command = SpawnBatchCommand::new(py, components, count)?;
        let trace_payloads = self.prepare_uniform_batch_trace_payloads(components)?;

        if self.is_world {
            self.check_native_asset_access("commands.spawn_batch()")?;
            let entities = command.apply(self.world_mut()?)?;
            let entity_list: Vec<PyEntity> = entities.into_iter().map(PyEntity).collect();
            Ok(entity_list.into_pyobject(py)?.into())
        } else {
            // Reserve IDs at the adapter boundary so parity tracing can assign
            // stable spawn tokens before the flush resolves raw targets.
            let entities = {
                let commands = self.commands_mut()?;
                (0..command.spawn_count())
                    .map(|_| commands.spawn_empty().id())
                    .collect::<Vec<_>>()
            };
            for &entity in &entities {
                self.trace_spawn(entity);
            }
            let error_sink = self.error_sink.clone().ok_or_else(|| {
                PyRuntimeError::new_err("Deferred spawn_batch requires an App-owned error sink")
            })?;
            let trace_entities = entities.clone();
            self.commands_mut()?.queue(move |world: &mut World| {
                if let Err(e) = command.apply_to(world, entities) {
                    error_sink.record(e);
                }
            });
            self.record_batch_inserts(&trace_entities, &trace_payloads);
            Ok(py.None())
        }
    }

    #[pyo3(name = "_spawn_batch_iter")]
    fn spawn_batch_iter(&self, py: Python, batch: &Bound<'_, PyAny>) -> PyResult<()> {
        let prepared = prepare_iter_batch(py, batch)?;
        if self.is_world {
            self.check_native_asset_access("commands.spawn_batch()")?;
            let world = self.world_mut()?;
            for command in prepared {
                for entity in command.apply(world)? {
                    self.trace_spawn(entity);
                }
            }
            return Ok(());
        }

        let mut batches = Vec::with_capacity(prepared.len());
        for command in prepared {
            let entities = {
                let commands = self.commands_mut()?;
                (0..command.spawn_count())
                    .map(|_| commands.spawn_empty().id())
                    .collect::<Vec<_>>()
            };
            for &entity in &entities {
                self.trace_spawn(entity);
            }
            batches.push((command, entities));
        }
        let error_sink = self.error_sink.clone().ok_or_else(|| {
            PyRuntimeError::new_err("Deferred spawn_batch requires an App-owned error sink")
        })?;
        self.commands_mut()?.queue(move |world: &mut World| {
            for (command, entities) in batches {
                if let Err(error) = command.apply_to(world, entities) {
                    error_sink.record(error);
                    break;
                }
            }
        });
        Ok(())
    }

    pub fn entity(&self, entity: &PyEntity) -> PyResult<PyEntityCommands> {
        self.check_valid()?;
        if self.is_world {
            let world = self.world_mut()?;
            if world.get_entity(entity.0).is_err() {
                return Err(PyValueError::new_err("Entity does not exist in the world"));
            }
        }
        // Note: For Commands backend, we can't check existence (deferred operations)
        Ok(PyEntityCommands::with_commands(entity.0, self))
    }

    pub fn get_entity(&self, entity: &PyEntity) -> PyResult<Option<PyEntityCommands>> {
        self.check_valid()?;
        if self.is_world {
            let world = self.world_mut()?;
            if world.get_entity(entity.0).is_err() {
                return Ok(None);
            }
        }
        Ok(Some(PyEntityCommands::with_commands(entity.0, self)))
    }

    pub fn despawn(&self, entity: &PyEntity) -> PyResult<()> {
        self.check_valid()?;
        self.check_native_asset_access("commands.despawn()")?;
        let entity_id = entity.0;
        self.trace_target_op(ParityOpKind::Despawn, entity_id);

        if self.is_world {
            let world = self.world_mut()?;
            if hierarchy_contains_resource_entity(world, entity_id) {
                return Err(PyTypeError::new_err(RESOURCE_ENTITY_DESPAWN));
            }
            crate::ecs::lifecycle_mutation::despawn_recursive(world, entity_id);
        } else {
            // Deferred commands
            // We need to collect component types before queuing the despawn
            // This is tricky because we can't access the world yet
            // For now, we'll collect component types in the deferred command
            let error_sink = self.error_sink.clone();
            self.execute_or_queue(move |world| {
                // The world is only reachable at flush time, so a resource-entity
                // despawn reports through the system error sink instead of the
                // queuing call.
                if hierarchy_contains_resource_entity(world, entity_id) {
                    report_deferred_error(
                        &error_sink,
                        "Failed to despawn via Commands",
                        PyTypeError::new_err(RESOURCE_ENTITY_DESPAWN),
                    );
                    return;
                }
                crate::ecs::lifecycle_mutation::despawn_recursive(world, entity_id);
            })?;
        }

        Ok(())
    }

    pub fn insert_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;

        let trace_operation =
            self.prepare_trace_op(ParityOpKind::ResourceInsert, &resource, None)?;

        // Get the resource type from the instance
        let resource_type = resource.get_type();
        let py_resource_type = PyResourceType::try_from((&resource_type, py))?;

        // Convert the bound resource to a Py<PyAny>
        let resource_instance: Py<PyAny> = resource.unbind();
        self.check_native_asset_access("commands.insert_resource()")?;

        if self.is_world {
            // Direct insertion into world
            py_resource_type.insert_into_world(self.world_mut()?, py, resource_instance)?;
        } else {
            // Queue a command to insert the resource later
            // Clone resource_instance for the command closure
            let resource_clone = resource_instance.clone_ref(py);
            let error_sink = self.error_sink.clone();

            self.execute_or_queue(move |world: &mut World| {
                Python::attach(|py| {
                    if let Err(e) = py_resource_type.insert_into_world(world, py, resource_clone) {
                        report_deferred_error(
                            &error_sink,
                            "Failed to insert resource via Commands",
                            e,
                        );
                    }
                });
            })?;
        }

        self.record_prepared_trace_op(trace_operation);

        Ok(())
    }

    pub fn remove_resource(&self, py: Python, resource_type: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;

        // Get the resource type - it should be a type object
        let type_obj = resource_type.cast::<PyType>().map_err(|_| {
            PyTypeError::new_err("remove_resource expects a resource type (class), not an instance")
        })?;

        let py_resource_type = PyResourceType::try_from((type_obj, py))?;
        let trace_operation = if self.parity_trace.is_some() {
            Some(PendingParityOp {
                kind: ParityOpKind::ResourceRemove,
                type_name: Some(type_obj.name()?.to_string()),
                payload_digest: CanonValue::None.digest(),
                target: None,
            })
        } else {
            None
        };
        self.check_native_asset_access("commands.remove_resource()")?;

        if self.is_world {
            // Direct removal from world
            py_resource_type.remove_from_world(self.world_mut()?, py)?;
        } else {
            // Queue a command to remove the resource later
            let error_sink = self.error_sink.clone();
            self.execute_or_queue(move |world: &mut World| {
                Python::attach(|py| {
                    if let Err(e) = py_resource_type.remove_from_world(world, py) {
                        report_deferred_error(
                            &error_sink,
                            "Failed to remove resource via Commands",
                            e,
                        );
                    }
                });
            })?;
        }

        self.record_prepared_trace_op(trace_operation);

        Ok(())
    }

    pub fn trigger(&self, py: Python, event: Bound<'_, PyAny>) -> PyResult<()> {
        let target_entity = if event.hasattr("entity")? {
            Some(event.getattr("entity")?.extract::<PyEntity>()?.0)
        } else {
            None
        };
        trigger_event_helper(self, py, event, target_entity)
    }
}
