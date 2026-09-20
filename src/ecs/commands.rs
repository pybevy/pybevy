use std::{
    any::TypeId,
    hash::Hash,
    ptr,
    sync::{Arc, Mutex, OnceLock},
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
    ComponentBridge, LogicalTypeId, LogicalTypeMap, PreparedUniformComponent,
    PyLogicalComponentParam,
    component_layout::ComponentLayout,
    custom_resource::validate_hierarchy_link,
    ensure_no_live_asset_access, extract_entity_from_any,
    public_error::{
        ASSET_SERVER_MANUAL_INSERT, ASSET_SERVER_MANUAL_REMOVE, IS_RESOURCE_COMPONENT_REMOVE,
        RESOURCE_COMPONENT_INSERT, RESOURCE_COMPONENT_REMOVE, RESOURCE_COMPONENT_SPAWN,
        RESOURCE_ENTITY_DESPAWN,
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
    intern,
    prelude::*,
    types::{PyTuple, PyType},
};
use smallvec::SmallVec;

use super::{
    PyEntity,
    component_type::{
        PreparedCustomComponentRegistration, PyComponentType, ValidationIdentity,
        register_prepared_custom_component,
    },
    entity_commands::PyEntityCommands,
    helpers::validity_guard::ValidityFlag,
    resource::hierarchy_contains_resource_entity,
    resource_type::PyResourceType,
    world::PyWorld,
    world_commands::{self, WorldCommand, WorldMutation},
    world_gc::WorldGcState,
};
use crate::ecs::{
    batch_spawn::{SpawnBatchCommand, prepare_iter_batch},
    component_layout::{ComponentStorageType, serialize_to_wrapper},
    component_wrapper::*,
    dynamic_system::{
        BufferedSystemError, SystemErrorBuffer, SystemErrorReport, lock_or_recover,
        print_reported_system_error_once,
    },
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
    if commands.is_world_queue() {
        commands.queue_world(WorldCommand::Trigger(target_entity, event.unbind()))?;
        commands.record_prepared_trace_op(trace_operation);
        return Ok(());
    }
    let event_clone = event.clone().unbind();

    if commands.is_immediate() {
        let mut world = commands.world_mut()?;
        ensure_no_live_asset_access(&world, "commands.trigger()")
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let observers = world
            .get_resource::<ObserverRegistry>()
            .map(|registry| registry.snapshot_user_event(&event, target_entity))
            .unwrap_or_default();
        for observer_entry in observers {
            if !ObserverRegistry::matches_user_filter(&observer_entry, &world, target_entity) {
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
                &mut world,
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

/// Resolved component types for one bundle. Bundles are small in practice, so
/// this stays on the stack rather than allocating per spawn.
pub(crate) type ResolvedComponentTypes = SmallVec<[PyComponentType; 8]>;

pub(crate) fn resolve_spawn_bundle(
    py: Python,
    components: &Bound<'_, PyTuple>,
) -> PyResult<ResolvedComponentTypes> {
    resolve_component_bundle(py, components, true)
}

pub(crate) fn resolve_insert_bundle(
    py: Python,
    components: &Bound<'_, PyTuple>,
) -> PyResult<ResolvedComponentTypes> {
    resolve_component_bundle(py, components, false)
}

fn resolve_component_bundle(
    py: Python,
    components: &Bound<'_, PyTuple>,
    reject_resources: bool,
) -> PyResult<ResolvedComponentTypes> {
    let mut component_types = ResolvedComponentTypes::with_capacity(components.len());
    let mut identities = SmallVec::<[ValidationIdentity; 8]>::with_capacity(components.len());
    for component in components.iter() {
        let (component_type, identity) =
            PyComponentType::resolve_with_identity(&component.get_type(), py)?;
        if reject_resources && matches!(component_type, PyComponentType::Resource(_)) {
            return Err(PyTypeError::new_err(RESOURCE_COMPONENT_SPAWN));
        }
        component_types.push(component_type);
        identities.push(identity);
    }
    reject_duplicate_components(components, identities)?;
    Ok(component_types)
}

fn reject_duplicate_components<K: Eq + Hash>(
    components: &Bound<'_, PyTuple>,
    keys: impl IntoIterator<Item = K>,
) -> PyResult<()> {
    let Some((first, duplicate)) = first_duplicate_indices(keys) else {
        return Ok(());
    };
    let name_at = |index: usize| -> PyResult<String> {
        Ok(components.get_item(index)?.get_type().name()?.to_string())
    };
    Err(PyValueError::new_err(format!(
        "component bundle contains duplicate '{}'; it was already supplied as '{}'",
        name_at(duplicate)?,
        name_at(first)?
    )))
}

#[derive(Clone)]
pub(crate) struct CommandErrorSink {
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
    error_report: SystemErrorReport,
    retain_exception: bool,
}

impl CommandErrorSink {
    pub(crate) fn new(
        error_state: Arc<Mutex<Vec<PyErr>>>,
        error_buffer: SystemErrorBuffer,
        error_report: SystemErrorReport,
        retain_exception: bool,
    ) -> Self {
        Self {
            error_state,
            error_buffer,
            error_report,
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
            if !self.retain_exception {
                print_reported_system_error_once(
                    &self.error_report,
                    "deferred Python command",
                    &message,
                    traceback.as_deref(),
                );
            }
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
///
/// Invariant: no method here may take `&mut self` or call `borrow_mut()` on
/// this type. PyO3 holds a PyRef on the receiver for the whole body of a
/// `&self` method, and these bodies call back into Python (component
/// conversion, and the `with_children` callback further down the handle
/// chain), so that Python can re-enter this object and borrow it again. A
/// mutable receiver would raise BorrowMutError instead. Mutation goes through
/// the validity-checked raw pointer behind `&self`.
/// See docs/safety.md, "Shared-Borrow Proxies"; `pybevy_lint` W014 guards it.
#[pyclass(name = "Commands", module = "pybevy.ecs")]
pub struct PyCommands {
    source: CommandSource,
    // Keep the PyWorld alive if we're wrapping a World
    _world_ref: Option<Py<PyWorld>>,
    // Runtime validity check - prevents use after system execution
    validity: ValidityFlag,
    error_sink: Option<CommandErrorSink>,
    parity_trace: Option<ParityRunHandle>,
    gc_state: Option<WorldGcState>,

    /// Cache one retained class because repeated spawns otherwise rebuild its layout.
    wrapper_layout_cache: OnceLock<CachedCustomComponentLayout>,
}

#[derive(Clone, Copy)]
enum CommandSource {
    Immediate(*mut World),
    WorldQueue(*mut World),
    System(*mut Commands<'static, 'static>),
    Structural(*mut CommandQueue),
}

struct CachedCustomComponentLayout {
    type_id: usize,
    retained_type: Py<PyType>,
    storage: ComponentStorageType,
    layout: Option<Arc<ComponentLayout>>,
}

// SAFETY: pointer access checks validity, execution scope and origin thread;
// owned World handles retain their allocation and scoped handles expire after use.
unsafe impl Send for PyCommands {}

// SAFETY: validity rejects cross-thread and reentrant access to each command source.
unsafe impl Sync for PyCommands {}

impl PyCommands {
    pub(crate) fn clone_for_handle(&self, py: Python<'_>) -> Self {
        Self {
            source: self.source,
            _world_ref: self._world_ref.as_ref().map(|world| world.clone_ref(py)),
            validity: self.validity.clone(),
            error_sink: self.error_sink.clone(),
            parity_trace: self.parity_trace.clone(),
            gc_state: self.gc_state.clone(),
            wrapper_layout_cache: OnceLock::new(),
        }
    }

    pub(crate) fn traverse_owner(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self._world_ref)?;
        if let Some(cached) = self.wrapper_layout_cache.get() {
            visit.call(&cached.retained_type)?;
        }
        Ok(())
    }

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
            source: CommandSource::System(ptr::from_mut(commands).cast()),
            wrapper_layout_cache: OnceLock::new(),
            _world_ref: None,
            validity,
            error_sink: Some(error_sink),
            parity_trace,
            gc_state: None,
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
        let (gc_state, parity_trace) = Python::attach(|py| {
            let world = world_ref.borrow(py);
            (world.gc_state(), world.parity_trace.clone())
        });
        Self {
            source: CommandSource::Immediate(world_ptr),
            wrapper_layout_cache: OnceLock::new(),
            _world_ref: Some(world_ref),
            validity,
            error_sink: None,
            parity_trace,
            gc_state,
        }
    }

    pub(crate) unsafe fn from_world_queue(
        world_ptr: *mut World,
        world_ref: Py<PyWorld>,
        validity: ValidityFlag,
    ) -> Self {
        // SAFETY: the caller supplies the retained World's live exclusive capability.
        let mut commands = unsafe { Self::from_world(world_ptr, world_ref, validity) };
        commands.source = CommandSource::WorldQueue(world_ptr);
        commands
    }

    fn is_immediate(&self) -> bool {
        matches!(self.source, CommandSource::Immediate(_))
    }

    pub(crate) fn is_world_queue(&self) -> bool {
        matches!(self.source, CommandSource::WorldQueue(_))
    }

    pub(crate) fn error_boundary(&self) -> PyResult<world_commands::ErrorBoundary> {
        self.check_valid()?;
        if self.is_immediate() && self._world_ref.is_some() {
            Ok(world_commands::ErrorBoundary::new(&*self.world_mut()?))
        } else {
            Ok(world_commands::ErrorBoundary::default())
        }
    }

    pub(crate) fn queue_world(&self, command: WorldCommand) -> PyResult<()> {
        world_commands::enqueue(&mut *self.world_mut()?, command);
        Ok(())
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
        // SAFETY: the caller supplies a live World and exclusive access to its metadata.
        let gc_state = WorldGcState::for_world(unsafe { (&*world_ptr).id() });
        Self {
            source: CommandSource::Immediate(world_ptr),
            wrapper_layout_cache: OnceLock::new(),
            _world_ref: None,
            validity,
            error_sink: None,
            parity_trace: None,
            gc_state,
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
            source: CommandSource::Structural(queue),
            _world_ref: None,
            validity,
            error_sink,
            parity_trace: None,
            gc_state: None,
            wrapper_layout_cache: OnceLock::new(),
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

    fn with_commands<T>(&self, operation: impl FnOnce(&mut Commands) -> T) -> PyResult<T> {
        self.validity.check()?;
        match self.source {
            CommandSource::System(pointer) => {
                // SAFETY: the run validity fences the injected Commands allocation.
                Ok(operation(unsafe { &mut *pointer }))
            }
            CommandSource::WorldQueue(_) => {
                let mut world = self.world_mut()?;
                Ok(operation(&mut world.commands()))
            }
            _ => Err(PyRuntimeError::new_err(
                "This command source cannot reserve entities",
            )),
        }
    }

    // Raw pointer access behind the validity check, see docs/safety.md.
    // The returned guard's scope is a native mutation window: deferred Python
    // drops from the previous mutation drain before it, and drops caused by
    // this window drain when the borrow ends, before returning to Python.
    fn world_mut(&self) -> PyResult<crate::ecs::deferred_drop::WorldMutGuard<'_>> {
        self.validity.check()?;
        let pointer = match self.source {
            CommandSource::Immediate(pointer) | CommandSource::WorldQueue(pointer) => pointer,
            _ => {
                return Err(PyRuntimeError::new_err(
                    "Cannot get World from Commands-backed PyCommands",
                ));
            }
        };
        let gc = self.gc_state.as_ref().map(WorldGcState::suspend);
        // SAFETY: validity and the GC gate protect the live World mutation window.
        let world = unsafe { &mut *pointer };
        Ok(crate::ecs::deferred_drop::WorldMutGuard::with_gc(world, gc))
    }

    pub(crate) fn check_native_asset_access(&self, operation: &str) -> PyResult<()> {
        if !self.is_immediate() {
            return Ok(());
        }
        let world = self.world_mut()?;
        ensure_no_live_asset_access(&world, operation)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    /// Get world access if this is world-backed, otherwise return None.
    pub(crate) fn try_world_mut(
        &self,
    ) -> PyResult<Option<crate::ecs::deferred_drop::WorldMutGuard<'_>>> {
        self.validity.check()?;
        if self.is_immediate() {
            self.world_mut().map(Some)
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
        if self.is_immediate() {
            let mut world = self.world_mut()?;
            operation(&mut world);
        } else if let CommandSource::Structural(pointer) = self.source {
            self.validity.check()?;
            // SAFETY: `from_queue_temporary` provides the only live mutable
            // access and the validity fence covers this append.
            unsafe { &mut *pointer }.push(operation);
        } else {
            self.with_commands(|commands| commands.queue(operation))?;
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
        if self.is_immediate() {
            Ok(world_op(&mut *self.world_mut()?))
        } else if matches!(self.source, CommandSource::Structural(_)) {
            Err(PyRuntimeError::new_err(
                "This temporary command queue cannot reserve or return entities",
            ))
        } else {
            self.with_commands(commands_op)
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
    let Ok(Some(value)) = component.getattr_opt(intern!(component.py(), "_logical_type_id")) else {
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

enum PreparedComponentValue {
    Native {
        value: Box<dyn PreparedUniformComponent>,
        parent: Option<Entity>,
        logical_type: Option<Option<LogicalTypeId>>,
    },
    Custom(
        PreparedCustomComponentRegistration,
        PreparedCustomComponentValue,
    ),
}

pub(crate) struct PreparedInsertion {
    types: ResolvedComponentTypes,
    classes: Vec<Py<PyType>>,
    values: Vec<PreparedComponentValue>,
}

impl PreparedInsertion {
    fn new(
        commands: &PyCommands,
        components: &Bound<'_, PyTuple>,
        types: ResolvedComponentTypes,
    ) -> PyResult<Self> {
        let mut classes = Vec::with_capacity(components.len());
        let mut values = Vec::with_capacity(components.len());
        for (component, kind) in components.iter().zip(&types) {
            classes.push(component.get_type().unbind());
            values.push(match kind {
                PyComponentType::Dynamic(pointer) => {
                    let bridge =
                        global_registry::get_bridge_by_py_type(*pointer).ok_or_else(|| {
                            PyRuntimeError::new_err("Dynamic component type not registered")
                        })?;
                    let parent = bridge
                        .relationship_field()
                        .map(|field| -> PyResult<Entity> {
                            Ok(component.getattr(field)?.extract::<PyEntity>()?.0)
                        })
                        .transpose()?;
                    PreparedComponentValue::Native {
                        value: bridge.prepare_uniform(&component)?,
                        parent,
                        logical_type: component_logical_type(&component)?,
                    }
                }
                PyComponentType::Custom(_) => {
                    let (registration, value) = prepare_custom_component(commands, &component)?;
                    PreparedComponentValue::Custom(registration, value)
                }
                PyComponentType::Resource(_) => {
                    return Err(PyTypeError::new_err(RESOURCE_COMPONENT_INSERT));
                }
            });
        }
        Ok(Self {
            types,
            classes,
            values,
        })
    }

    pub(crate) fn traverse(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.classes {
            visit.call(class)?;
        }
        for value in &self.values {
            match value {
                PreparedComponentValue::Native { value, .. } => value.traverse(visit.clone())?,
                PreparedComponentValue::Custom(registration, value) => {
                    registration.traverse(visit.clone())?;
                    match value {
                        PreparedCustomComponentValue::Wrapper { retained_type, .. } => {
                            visit.call(retained_type)?
                        }
                        PreparedCustomComponentValue::PyObject(value) => visit.call(value)?,
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn apply(self, world: &mut World, entity: Entity) -> PyResult<()> {
        Python::attach(|py| {
            for value in &self.values {
                if let PreparedComponentValue::Custom(registration, _) = value {
                    registration.validate_current(py)?;
                }
            }
            Ok::<(), PyErr>(())
        })?;
        ensure_entity_exists(world, entity)?;
        let mut result = Ok(());
        crate::ecs::lifecycle_mutation::insert_many_with(world, entity, &self.types, |world| {
            result = Python::attach(|py| {
                for (kind, mut value) in self.types.iter().zip(self.values) {
                    ensure_entity_exists(world, entity)?;
                    match &mut value {
                        PreparedComponentValue::Native {
                            value,
                            parent,
                            logical_type,
                        } => {
                            if let Some(parent) = parent {
                                validate_hierarchy_link(world, entity, *parent)
                                    .map_err(|error| PyTypeError::new_err(error.to_string()))?;
                            }
                            let id = kind.register_simple(world, py);
                            value.insert(id, &[entity], world);
                            if entity_exists(world, entity)
                                && let (Some(logical), Some(native)) =
                                    (logical_type, kind.type_id())
                            {
                                update_entity_logical_type(world, entity, native, *logical);
                            }
                        }
                        PreparedComponentValue::Custom(registration, value) => {
                            let id = register_prepared_custom_component(world, registration);
                            match value {
                                PreparedCustomComponentValue::Wrapper {
                                    bytes,
                                    wrapper_size,
                                    ..
                                } => {
                                    insert_custom_wrapper_bytes(
                                        world,
                                        entity,
                                        id,
                                        *wrapper_size,
                                        bytes,
                                    );
                                }
                                PreparedCustomComponentValue::PyObject(value) => {
                                    OwningPtr::make(value.clone_ref(py), |pointer| {
                                        // SAFETY: prepared registration uses exactly Py<PyAny> storage.
                                        unsafe {
                                            world.entity_mut(entity).insert_by_id(id, pointer);
                                        }
                                    });
                                }
                            }
                        }
                    }
                }
                Ok(())
            });
            result.is_ok()
        });
        result
    }
}

impl PyCommands {
    /// Reuse an unchanged class layout without weakening live-schema validation.
    fn storage_and_layout(
        &self,
        cls: &Bound<'_, PyType>,
    ) -> PyResult<(ComponentStorageType, Option<Arc<ComponentLayout>>)> {
        let key = cls.as_type_ptr() as usize;
        // Re-read live metadata so class mutations cannot reuse a stale layout.
        if let Some(cached) = self.wrapper_layout_cache.get()
            && cached.type_id == key
            && crate::ecs::component_layout::cached_storage_and_layout_match(
                cls,
                cached.storage,
                cached.layout.as_deref(),
            )?
        {
            return Ok((cached.storage, cached.layout.clone()));
        }

        let (storage, layout) = crate::ecs::component_type::storage_and_shared_layout(cls)?;
        if self.wrapper_layout_cache.get().is_none() {
            let cached = CachedCustomComponentLayout {
                type_id: key,
                retained_type: cls.clone().unbind(),
                storage,
                layout: layout.clone(),
            };
            if self.wrapper_layout_cache.set(cached).is_ok() {
                let cached = self
                    .wrapper_layout_cache
                    .get()
                    .expect("a successful OnceLock set stores the layout");
                return Ok((cached.storage, cached.layout.clone()));
            }
        }
        Ok((storage, layout))
    }
}

fn prepare_custom_component(
    commands: &PyCommands,
    component: &Bound<'_, PyAny>,
) -> PyResult<(
    PreparedCustomComponentRegistration,
    PreparedCustomComponentValue,
)> {
    let component_type = component.get_type();
    let (storage, layout) = commands.storage_and_layout(&component_type)?;
    let registration = PreparedCustomComponentRegistration::from_python_class_with_layout(
        &component_type,
        storage,
        layout,
    )?;
    let value = match registration.storage_type() {
        ComponentStorageType::Wrapper(wrapper_size) => {
            let layout = registration
                .wrapper_layout()
                .expect("wrapper storage carries the layout that selected it");
            let bytes = serialize_to_wrapper(component, layout)?;
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
    let component_types = resolve_insert_bundle(py, components)?;
    insert_resolved_components_to_entity(commands, entity_id, components, component_types)
}

pub(crate) fn insert_resolved_components_to_entity(
    commands: &PyCommands,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
    component_types: ResolvedComponentTypes,
) -> PyResult<()> {
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

    if commands.is_world_queue() {
        let insertion = PreparedInsertion::new(commands, components, component_types)?;
        commands.queue_world(WorldCommand::Insert(entity_id, Box::new(insertion)))?;
        for operation in trace_operations {
            commands.record_prepared_trace_op(operation);
        }
        return Ok(());
    }

    if component_types.is_empty() {
        insert_components_to_entity(commands, commands, entity_id, components, &component_types)?;
        for operation in trace_operations {
            commands.record_prepared_trace_op(operation);
        }
        return Ok(());
    }

    if commands.is_immediate() {
        commands.check_native_asset_access("entity.insert()")?;
        let validity = commands.validity.clone();
        let mut world = commands.world_mut()?;
        ensure_entity_exists(&world, entity_id)?;
        let mut insertion_error = None;
        crate::ecs::lifecycle_mutation::insert_many_with(
            &mut world,
            entity_id,
            &component_types,
            |world| {
                // SAFETY: this temporary wrapper is a reborrow of the
                // adapter's exclusive World for the structural commit only.
                // It cannot escape this closure and is never used alongside
                // the adapter's `&mut World`.
                let temporary =
                    unsafe { PyCommands::from_world_temporary(world as *mut World, validity) };
                match insert_components_to_entity(
                    &temporary,
                    commands,
                    entity_id,
                    components,
                    &component_types,
                ) {
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
        // SAFETY: the adapter is confined to this live local queue and validity scope.
        let temporary = unsafe {
            PyCommands::from_queue_temporary(
                &mut structural_queue,
                commands.validity.clone(),
                commands.error_sink.clone(),
            )
        };
        insert_components_to_entity(
            &temporary,
            commands,
            entity_id,
            components,
            &component_types,
        )?;
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
    layout_cache_owner: &PyCommands,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
    component_types: &[PyComponentType],
) -> PyResult<()> {
    debug_assert_eq!(components.len(), component_types.len());
    if commands.is_immediate() {
        let world = commands.world_mut()?;
        ensure_entity_exists(&world, entity_id)?;
    }

    for (component, component_type) in components.iter().zip(component_types) {
        // Insert the component based on its type
        match *component_type {
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

                if commands.is_immediate() {
                    // Direct world access - insert immediately via bridge
                    let mut world = commands.world_mut()?;
                    validate_relationship_component(
                        &world,
                        entity_id,
                        &component,
                        bridge.as_ref(),
                    )?;
                    bridge.insert(&mut world, entity_id, &component)?;
                    if let Some(logical_type) = logical_type {
                        update_entity_logical_type(
                            &mut world,
                            entity_id,
                            native_type,
                            logical_type,
                        );
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
                let (registration, prepared_value) =
                    prepare_custom_component(layout_cache_owner, &component)?;

                if commands.is_immediate() {
                    let mut world = commands.world_mut()?;
                    let component_id =
                        register_prepared_custom_component(&mut world, &registration);
                    match prepared_value {
                        PreparedCustomComponentValue::Wrapper {
                            bytes,
                            wrapper_size,
                            ..
                        } => insert_custom_wrapper_bytes(
                            &mut world,
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
    if commands.is_world_queue() {
        return commands.queue_world(WorldCommand::Mutation(WorldMutation::AddChild(
            parent_id, child_id,
        )));
    }
    if commands.is_immediate() {
        let mut world = commands.world_mut()?;
        ensure_entities_exist(&world, &[parent_id, child_id])?;
        validate_hierarchy_link(&world, child_id, parent_id)
            .map_err(|error| PyTypeError::new_err(error.to_string()))?;
        ensure_no_live_asset_access(&world, "entity.add_child()")
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
    if commands.is_world_queue() {
        return commands.queue_world(WorldCommand::Mutation(WorldMutation::RemoveChildren(
            parent_id,
            child_ids.to_vec(),
        )));
    }
    if commands.is_immediate() {
        let mut world = commands.world_mut()?;
        ensure_entity_exists(&world, parent_id)?;
        ensure_entities_exist(&world, child_ids)?;
        ensure_no_live_asset_access(&world, "entity.remove_children()")
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
    if commands.is_world_queue() {
        return commands.queue_world(WorldCommand::Mutation(WorldMutation::ClearChildren(
            parent_id,
        )));
    }
    if commands.is_immediate() {
        let mut world = commands.world_mut()?;
        ensure_entity_exists(&world, parent_id)?;
        ensure_no_live_asset_access(&world, "entity.clear_children()")
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
    if commands.is_world_queue() {
        return commands.queue_world(WorldCommand::Mutation(WorldMutation::SetParent(
            child_id, parent_id,
        )));
    }
    if commands.is_immediate() {
        let mut world = commands.world_mut()?;
        ensure_entities_exist(&world, &[child_id, parent_id])?;
        validate_hierarchy_link(&world, child_id, parent_id)
            .map_err(|error| PyTypeError::new_err(error.to_string()))?;
        ensure_no_live_asset_access(&world, "entity.set_parent()")
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
    if commands.is_world_queue() {
        return commands.queue_world(WorldCommand::Mutation(WorldMutation::RemoveParent(
            child_id,
        )));
    }
    if commands.is_immediate() {
        let mut world = commands.world_mut()?;
        ensure_entity_exists(&world, child_id)?;
        ensure_no_live_asset_access(&world, "entity.remove_parent()")
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
        if commands.is_world_queue() {
            continue;
        }
        if commands.is_immediate() {
            let mut world = commands.world_mut()?;
            ensure_entity_exists(&world, entity_id)?;
            if let Some(logical_type) = logical_type
                && !entity_logical_type_matches(&world, entity_id, component_type, logical_type)
            {
                continue;
            }
            ensure_no_live_asset_access(&world, "entity.remove()")
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            crate::ecs::lifecycle_mutation::remove(&mut world, entity_id, component_type);
            if let Some(native_type) = component_type.type_id() {
                update_entity_logical_type(&mut world, entity_id, native_type, None);
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

    if commands.is_world_queue() {
        commands.queue_world(WorldCommand::Remove(entity_id, components.clone().unbind()))?;
    }

    Ok(())
}

#[pymethods]
impl PyCommands {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        self.traverse_owner(visit)
    }

    pub fn spawn_empty(&self, _py: Python<'_>) -> PyResult<PyEntityCommands> {
        self.error_boundary()?.run(|| {
            self.check_valid()?;
            self.check_native_asset_access("commands.spawn_empty()")?;

            let entity = self.execute_returning(
                |world| world.spawn_empty().id(),
                |commands| commands.spawn_empty().id(),
            )?;
            self.trace_spawn(entity);

            Ok(PyEntityCommands::with_commands(entity, self, _py))
        })
    }

    #[pyo3(signature = (*components))]
    pub fn spawn(&self, py: Python, components: &Bound<'_, PyTuple>) -> PyResult<PyEntityCommands> {
        self.error_boundary()?.run(|| {
            self.check_valid()?;
            let components_to_insert = normalize_spawn_components(components)?;
            let component_types = resolve_spawn_bundle(py, &components_to_insert)?;
            self.check_native_asset_access("commands.spawn()")?;

            let entity_id = self.execute_returning(
                |world| world.spawn_empty().id(),
                |commands| commands.spawn_empty().id(),
            )?;
            self.trace_spawn(entity_id);

            insert_resolved_components_to_entity(
                self,
                entity_id,
                &components_to_insert,
                component_types,
            )?;

            Ok(PyEntityCommands::with_commands(entity_id, self, py))
        })
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

        // Reserve IDs at the adapter boundary so parity tracing can assign
        // stable spawn tokens before the flush resolves raw targets.
        let entities = self.with_commands(|commands| {
            (0..command.spawn_count())
                .map(|_| commands.spawn_empty().id())
                .collect::<Vec<_>>()
        })?;
        for &entity in &entities {
            self.trace_spawn(entity);
        }
        let trace_entities = entities.clone();
        if self.is_world_queue() {
            self.queue_world(WorldCommand::Batch(vec![(command, entities)]))?;
            self.record_batch_inserts(&trace_entities, &trace_payloads);
            return Ok(py.None());
        }
        let error_sink = self.error_sink.clone().ok_or_else(|| {
            PyRuntimeError::new_err("Deferred spawn_batch requires an App-owned error sink")
        })?;
        self.execute_or_queue(move |world: &mut World| {
            if let Err(e) = command.apply_to(world, entities) {
                error_sink.record(e);
            }
        })?;
        self.record_batch_inserts(&trace_entities, &trace_payloads);
        Ok(py.None())
    }

    #[pyo3(name = "_spawn_batch_iter")]
    fn spawn_batch_iter(&self, py: Python, batch: &Bound<'_, PyAny>) -> PyResult<()> {
        let prepared = prepare_iter_batch(py, batch)?;

        let mut batches = Vec::with_capacity(prepared.len());
        for command in prepared {
            let entities = self.with_commands(|commands| {
                (0..command.spawn_count())
                    .map(|_| commands.spawn_empty().id())
                    .collect::<Vec<_>>()
            })?;
            for &entity in &entities {
                self.trace_spawn(entity);
            }
            batches.push((command, entities));
        }
        if self.is_world_queue() {
            return self.queue_world(WorldCommand::Batch(batches));
        }
        let error_sink = self.error_sink.clone().ok_or_else(|| {
            PyRuntimeError::new_err("Deferred spawn_batch requires an App-owned error sink")
        })?;
        self.execute_or_queue(move |world: &mut World| {
            for (command, entities) in batches {
                if let Err(error) = command.apply_to(world, entities) {
                    error_sink.record(error);
                    break;
                }
            }
        })?;
        Ok(())
    }

    pub fn entity(&self, py: Python<'_>, entity: &Bound<'_, PyAny>) -> PyResult<PyEntityCommands> {
        let entity = &extract_entity_from_any(entity)?;
        self.check_valid()?;
        if self.is_immediate() {
            let world = self.world_mut()?;
            if world.get_entity(entity.0).is_err() {
                return Err(PyValueError::new_err("Entity does not exist in the world"));
            }
        }
        // Note: For Commands backend, we can't check existence (deferred operations)
        Ok(PyEntityCommands::with_commands(entity.0, self, py))
    }

    pub fn get_entity(
        &self,
        py: Python<'_>,
        entity: &Bound<'_, PyAny>,
    ) -> PyResult<Option<PyEntityCommands>> {
        let entity = &extract_entity_from_any(entity)?;
        self.check_valid()?;
        if self.is_immediate() {
            let world = self.world_mut()?;
            if world.get_entity(entity.0).is_err() {
                return Ok(None);
            }
        } else if !self.with_commands(|commands| commands.get_entity(entity.0).is_ok())? {
            return Ok(None);
        }
        Ok(Some(PyEntityCommands::with_commands(entity.0, self, py)))
    }

    pub fn despawn(&self, entity: &Bound<'_, PyAny>) -> PyResult<()> {
        self.error_boundary()?
            .run(|| self.despawn_entity(&extract_entity_from_any(entity)?))
    }

    pub fn insert_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<()> {
        self.error_boundary()?.run(|| {
            self.check_valid()?;

            let trace_operation =
                self.prepare_trace_op(ParityOpKind::ResourceInsert, &resource, None)?;

            // Get the resource type from the instance
            let resource_type = resource.get_type();
            let py_resource_type = PyResourceType::try_from((&resource_type, py))?;

            if self.is_world_queue() {
                let value = match &py_resource_type {
                    PyResourceType::Dynamic(pointer) => {
                        global_registry::get_resource_bridge_by_py_type(*pointer)
                            .ok_or_else(|| PyRuntimeError::new_err("Resource bridge not found"))?
                            .snapshot_for_commands(&resource)?
                    }
                    PyResourceType::Custom(_) => resource.unbind(),
                    PyResourceType::AssetServer => {
                        return Err(PyTypeError::new_err(ASSET_SERVER_MANUAL_INSERT));
                    }
                };
                self.queue_world(WorldCommand::InsertResource(value))?;
                self.record_prepared_trace_op(trace_operation);
                return Ok(());
            }

            // Convert the bound resource to a Py<PyAny>
            let resource_instance: Py<PyAny> = resource.unbind();
            self.check_native_asset_access("commands.insert_resource()")?;

            if self.is_immediate() {
                // Direct insertion into world
                py_resource_type.insert_into_world(
                    &mut *self.world_mut()?,
                    py,
                    resource_instance,
                )?;
            } else {
                // Queue a command to insert the resource later
                // Clone resource_instance for the command closure
                let resource_clone = resource_instance.clone_ref(py);
                let error_sink = self.error_sink.clone();

                self.execute_or_queue(move |world: &mut World| {
                    Python::attach(|py| {
                        if let Err(e) =
                            py_resource_type.insert_into_world(world, py, resource_clone)
                        {
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
        })
    }

    pub fn remove_resource(&self, py: Python, resource_type: Bound<'_, PyAny>) -> PyResult<()> {
        self.error_boundary()?.run(|| {
            self.check_valid()?;

            // Get the resource type - it should be a type object
            let type_obj = resource_type.cast::<PyType>().map_err(|_| {
                PyTypeError::new_err(
                    "remove_resource expects a resource type (class), not an instance",
                )
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
            if self.is_world_queue() {
                if matches!(py_resource_type, PyResourceType::AssetServer) {
                    return Err(PyTypeError::new_err(ASSET_SERVER_MANUAL_REMOVE));
                }
                self.queue_world(WorldCommand::RemoveResource(resource_type.unbind()))?;
                self.record_prepared_trace_op(trace_operation);
                return Ok(());
            }
            self.check_native_asset_access("commands.remove_resource()")?;

            if self.is_immediate() {
                // Direct removal from world
                py_resource_type.remove_from_world(&mut *self.world_mut()?, py)?;
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
        })
    }

    pub fn trigger(&self, py: Python, event: Bound<'_, PyAny>) -> PyResult<()> {
        self.error_boundary()?.run(|| {
            let target_entity = if event.hasattr("entity")? {
                Some(event.getattr("entity")?.extract::<PyEntity>()?.0)
            } else {
                None
            };
            trigger_event_helper(self, py, event, target_entity)
        })
    }
}

impl PyCommands {
    pub(crate) fn despawn_entity(&self, entity: &PyEntity) -> PyResult<()> {
        self.check_valid()?;
        self.check_native_asset_access("commands.despawn()")?;
        let entity_id = entity.0;
        self.trace_target_op(ParityOpKind::Despawn, entity_id);

        if self.is_world_queue() {
            return self.queue_world(WorldCommand::Mutation(WorldMutation::Despawn(entity_id)));
        }

        if self.is_immediate() {
            let mut world = self.world_mut()?;
            if hierarchy_contains_resource_entity(&world, entity_id) {
                return Err(PyTypeError::new_err(RESOURCE_ENTITY_DESPAWN));
            }
            crate::ecs::lifecycle_mutation::despawn_recursive(&mut world, entity_id);
        } else {
            let error_sink = self.error_sink.clone();
            self.execute_or_queue(move |world| {
                // Deferred failures report through the system error sink.
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
}
