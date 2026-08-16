use std::{cell::RefCell, collections::HashMap, ptr::NonNull, sync::Arc};

#[cfg(debug_assertions)]
use bevy::ecs::query::FilteredAccess;
use bevy::{
    ecs::{
        change_detection::Tick, component::ComponentId, world::unsafe_world_cell::UnsafeWorldCell,
    },
    prelude::*,
};
use pybevy_core::{
    ComponentWriteContext, ExtractFn, FilteredEntityAccess, LogicalTypeId, LogicalTypeMap,
    registry::global_registry,
};
use pybevy_ecs::shared::{
    cached_query::CachedQueryCore,
    query_builder_ext::{QueryBuildSpec, QueryComponent, QueryFilterBranch},
    query_runtime::{
        IterationToken, QueryExecutionError, QueryRuntimeCore, QueryRuntimeError, RowMaterializer,
    },
};
use pyo3::{
    IntoPyObjectExt, PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyStopIteration, PyTypeError},
    ffi::PyTypeObject,
    prelude::*,
    types::PyTuple,
};
use smallvec::SmallVec;

use crate::ecs::{
    PyEntity,
    commands::entity_logical_type_matches,
    component_layout::{ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt},
    component_type::{PyComponentType, register_component_id},
    filter::QueryFilter,
    helpers::validity_guard::{AccessMode, ValidityFlag},
    lazy_wrapper_proxy::{ProxyKind, PyLazyWrapperProxy},
    query::query_param::{PyQueryParam, QueryData},
    world::PyWorld,
};

fn resolve_or_branch(
    world: &mut World,
    filter: &QueryFilter,
    custom_component_ids: &HashMap<*const PyTypeObject, ComponentId>,
    py: Python<'_>,
) -> QueryFilterBranch {
    let mut branch = QueryFilterBranch::default();
    match filter {
        QueryFilter::With(filter) => {
            branch.with.extend(
                filter
                    .values
                    .iter()
                    .map(|ty| register_component_id(world, ty, custom_component_ids, py)),
            );
        }
        QueryFilter::Without(filter) => {
            branch.without.extend(
                filter
                    .values
                    .iter()
                    .map(|ty| register_component_id(world, ty, custom_component_ids, py)),
            );
        }
        QueryFilter::Changed(filter) => branch.changed.push(register_component_id(
            world,
            &filter.component_type,
            custom_component_ids,
            py,
        )),
        QueryFilter::Added(filter) => branch.added.push(register_component_id(
            world,
            &filter.component_type,
            custom_component_ids,
            py,
        )),
        QueryFilter::Or(_) => {
            unreachable!("Or construction accepts only simple query filters")
        }
    }
    branch
}

fn query_runtime_error_to_py(error: QueryRuntimeError) -> PyErr {
    match error {
        QueryRuntimeError::Storage(error) => error.into(),
        error => PyRuntimeError::new_err(error.to_string()),
    }
}

pub(crate) fn query_execution_error_to_py(error: QueryExecutionError<PyErr>) -> PyErr {
    match error {
        QueryExecutionError::Runtime(error) => query_runtime_error_to_py(error),
        QueryExecutionError::Materialize(error) => error,
    }
}

/// Static, per-Query-parameter state built once in `DynamicSystem::initialize` and
/// reused across every run. Owns the heavy `ErasedQueryState` plus the resolved
/// component-id caches, extraction function pointers, access modes and tick-filter
/// ids that would otherwise be recomputed on every `PyQueryIter::new`.
///
/// Stored on `DynamicSystem` (which the schedule owns and which outlives every run),
/// so `PyQueryIter` can borrow it via a raw pointer fenced by the same `ValidityFlag`
/// that fences the world cell.
pub struct CachedQuery {
    /// The query parameter information (shared via Arc to avoid clones).
    pub param: Arc<PyQueryParam>,

    /// The shared half: type-erased QueryState plus tick-filter ids
    /// (see `pybevy_ecs::shared::cached_query`).
    core: CachedQueryCore,

    /// Maps PyComponentType to their registered ComponentIds (cached for fast access).
    component_id_cache: HashMap<PyComponentType, ComponentId>,

    /// Maps custom component type pointers to their registered ComponentIds (shared via Arc).
    custom_component_ids: Arc<HashMap<*const PyTypeObject, ComponentId>>,

    /// Extraction function pointers for dynamic components, keyed by Python type.
    extract_fns: HashMap<PyComponentType, ExtractFn>,

    /// Declared read access for logical type value matching, when needed.
    logical_type_map_component_id: Option<ComponentId>,

    /// Whether this query was declared as `Single<T>` (enforces exactly one match).
    pub single_entity_enforced: bool,
}

// SAFETY: CachedQuery mirrors the Send/Sync discipline of the old PyQueryIter: the
// raw QueryState pointer and the Arc<HashMap> of type pointers are only ever touched
// while the owning DynamicSystem runs on a single thread. DynamicSystem is already
// declared Send + Sync; this makes the intent explicit for the standalone type.
unsafe impl Send for CachedQuery {}
unsafe impl Sync for CachedQuery {}

impl CachedQuery {
    /// Build the cached query state for a single Query parameter.
    ///
    /// This performs all the static, per-system work (component-id registration,
    /// filter-id collection, QueryState construction, extraction fn/access-mode
    /// arrays) that would otherwise run on every `PyQueryIter::new`. `initialize` legitimately
    /// holds `&mut World`, so building here is sound and happens once per parameter.
    pub fn build(
        world: &mut World,
        param: Arc<PyQueryParam>,
        custom_component_ids: Arc<HashMap<*const PyTypeObject, ComponentId>>,
        py: Python,
    ) -> Self {
        // Collect and register all component IDs (tracking optional and mutable status)
        let mut component_ids = Vec::new();
        let mut anyof_groups = Vec::new();
        for param_type in &param.data {
            match param_type {
                QueryData::Component {
                    ty: comp_type,
                    optional,
                    mutable,
                    ..
                } => {
                    let id = register_component_id(world, comp_type, &custom_component_ids, py);
                    component_ids.push(QueryComponent {
                        id,
                        optional: *optional,
                        mutable: *mutable,
                    });
                }
                QueryData::AnyOf { items } => {
                    let mut group = Vec::with_capacity(items.len());
                    for item in items {
                        let id = register_component_id(world, &item.ty, &custom_component_ids, py);
                        component_ids.push(QueryComponent {
                            id,
                            optional: true,
                            mutable: item.mutable,
                        });
                        group.push(id);
                    }
                    anyof_groups.push(group);
                }
                QueryData::Entity | QueryData::Has { .. } => {}
            }
        }
        let logical_type_map_component_id = param
            .data
            .iter()
            .any(|data| match data {
                QueryData::Component {
                    logical_type_id, ..
                } => logical_type_id.is_some(),
                QueryData::AnyOf { items } => {
                    items.iter().any(|item| item.logical_type_id.is_some())
                }
                QueryData::Entity | QueryData::Has { .. } => false,
            })
            .then(|| {
                let id = world.register_component::<LogicalTypeMap>();
                component_ids.push(QueryComponent {
                    id,
                    optional: true,
                    mutable: false,
                });
                id
            });

        // Collect filter component IDs
        let mut with_filter_ids = Vec::new();
        let mut without_filter_ids = Vec::new();
        let mut changed_filter_ids = Vec::new();
        let mut added_filter_ids = Vec::new();
        let mut or_filter_groups = Vec::new();

        for filter in &param.filters {
            match filter {
                QueryFilter::With(with) => {
                    for comp_type in &with.values {
                        let id = register_component_id(world, comp_type, &custom_component_ids, py);
                        with_filter_ids.push(id);
                    }
                }
                QueryFilter::Without(without) => {
                    for comp_type in &without.values {
                        let id = register_component_id(world, comp_type, &custom_component_ids, py);
                        without_filter_ids.push(id);
                    }
                }
                QueryFilter::Changed(changed) => {
                    let id = register_component_id(
                        world,
                        &changed.component_type,
                        &custom_component_ids,
                        py,
                    );
                    changed_filter_ids.push(id);
                }
                QueryFilter::Added(added) => {
                    let id = register_component_id(
                        world,
                        &added.component_type,
                        &custom_component_ids,
                        py,
                    );
                    added_filter_ids.push(id);
                }
                QueryFilter::Or(or) => {
                    or_filter_groups.push(
                        or.values
                            .iter()
                            .map(|filter| {
                                resolve_or_branch(world, filter, &custom_component_ids, py)
                            })
                            .collect(),
                    );
                }
            }
        }

        // Build the QueryState once. `build_auto` picks FilteredEntityRef for
        // all-read-only queries and retains the tick-filter ids for the
        // per-entity Changed/Added check.
        let spec = QueryBuildSpec {
            components: component_ids.clone(),
            with_filters: with_filter_ids,
            without_filters: without_filter_ids,
            changed_filters: changed_filter_ids,
            added_filters: added_filter_ids,
            anyof_groups,
            or_filters: or_filter_groups,
        };
        let core = CachedQueryCore::build_auto(world, &spec);

        // Cache every data type, including nested AnyOf items and Has data.
        let mut component_id_cache = HashMap::new();
        for ty in param.data.iter().flat_map(QueryData::component_types) {
            let component_id = register_component_id(world, &ty, &custom_component_ids, py);
            component_id_cache.insert(ty, component_id);
        }

        let mut extract_fns = HashMap::new();
        for ty in component_id_cache.keys() {
            if let PyComponentType::Dynamic(type_ptr) = ty
                && let Some(bridge) = global_registry::get_bridge_by_py_type(*type_ptr)
            {
                extract_fns.insert(*ty, bridge.extract_fn());
            }
        }

        let single_entity_enforced = param.single_entity_enforced;

        Self {
            param,
            core,
            component_id_cache,
            custom_component_ids,
            extract_fns,
            logical_type_map_component_id,
            single_entity_enforced,
        }
    }

    /// The QueryState's Bevy-computed `FilteredAccess`, exposed for the debug
    /// access auditor to compare against this system's declared access.
    #[cfg(debug_assertions)]
    pub(crate) fn component_access(&self) -> FilteredAccess {
        self.core.component_access()
    }
}

/// Runtime query iterator that can be passed to Python systems.
/// Uses Bevy's cached [`CachedQuery`] state for efficient iteration.
///
/// SAFETY: This struct erases the lifetimes of both the `UnsafeWorldCell` and the
/// borrowed `CachedQuery`. It must only be used within the scope of a system
/// execution and must not escape the Python GIL callback. Python code must not
/// store references to this object or any iterators derived from it beyond the
/// system function scope; the shared `ValidityFlag` fences any that leak.
///
/// # Performance Notes
/// For benchmarking, the main bottlenecks are typically:
/// 1. Bevy's `iter.next()` - iterating entities and fetching components
/// 2. `Py::new()` - creating Python wrapper objects (PyTransformMut, etc.)
/// 3. `clone_ref(py)` - Python reference counting overhead
/// 4. `PyTuple::new()` - allocating tuples for multi-component returns
/// 5. GIL acquisition/release (handled by PyO3 automatically)
#[pyclass(name = "QueryIter")]
pub struct PyQueryIter {
    /// The query parameter information: an `Arc` handle cloned from the cache at
    /// construction. Kept as a direct field so hot-path extraction reads
    /// `param.data` without dereferencing the raw `cached` pointer per entity.
    param: Arc<PyQueryParam>,

    /// Interpreter-neutral iteration, lookup, tick-filter, and validity state.
    ///
    /// Declared before `cached` so it drops first: for the Owned variant,
    /// `runtime` holds a raw pointer into the boxed CachedQuery, which would
    /// dangle during drop if the Box went away first.
    runtime: QueryRuntimeCore,

    cached: QueryCache,

    /// Reusable buffer for return values - avoids allocation on every __next__ call
    /// SmallVec[8] keeps up to 8 items on stack (most queries have 1-4 params)
    values_buffer: RefCell<SmallVec<[Py<PyAny>; 8]>>,

    /// Cached ComponentLayouts and storage types for custom wrapper components, keyed by type pointer.
    /// Avoids re-parsing Python __annotations__ and __pybevy_storage__ on every entity iteration.
    layout_cache: RefCell<
        HashMap<
            *const PyTypeObject,
            (
                crate::ecs::component_layout::ComponentStorageType,
                Option<Arc<crate::ecs::component_layout::ComponentLayout>>,
            ),
        >,
    >,
}

// SAFETY: PyQueryIter is only used during system execution on a single thread.
// QueryRuntimeCore fences the world cell and cached query state with the run's
// ValidityFlag; the remaining Python metadata follows the same discipline.
unsafe impl Send for PyQueryIter {}
// SAFETY: at most one IterationToken exists per runtime, and `__next__` takes
// `&mut self` on that iterator, so only one shared borrow can materialize rows.
// Every other Python-visible QueryIter operation takes `&mut self`; PyO3's
// borrow flag prevents those operations from overlapping the iterator's shared
// borrow. Any future `&self` QueryIter method that starts traversal or touches
// `values_buffer`/`layout_cache` must preserve this serialization argument.
unsafe impl Sync for PyQueryIter {}

enum QueryCache {
    Borrowed(NonNull<CachedQuery>),
    Owned(Box<CachedQuery>),
}

// SAFETY: borrowed caches follow the DynamicSystem validity window; owned
// caches remain pinned by Box for the lifetime of the iterator.
unsafe impl Send for QueryCache {}
// SAFETY: QueryRuntimeCore serializes traversal and fences World access.
unsafe impl Sync for QueryCache {}

impl QueryCache {
    fn get(&self) -> &CachedQuery {
        match self {
            // SAFETY: the borrowed pointer comes from `PyQueryIter::new`, whose
            // contract requires the CachedQuery to outlive this iterator; the
            // DynamicSystem owns it across every run.
            Self::Borrowed(cached) => unsafe { cached.as_ref() },
            Self::Owned(cached) => cached,
        }
    }

    fn is_owned(&self) -> bool {
        matches!(self, Self::Owned(_))
    }
}

/// Python iterator for one fresh traversal of a runtime Query.
#[pyclass(name = "QueryIterator")]
pub struct PyQueryIterator {
    query: Py<PyQueryIter>,
    state: QueryIteratorState,
}

enum QueryIteratorState {
    Runtime(IterationToken),
    Snapshot(std::vec::IntoIter<Entity>),
}

/// Entity ids that still honor the logical-type predicate.
///
/// Ad-hoc `World.query` resolves ids first and materializes components
/// afterwards, so without this it would admit entities whose logical
/// specialization a system query would have filtered out. Every ad-hoc entry
/// point (iteration, `get`, `iter_many`, `single`) must use this, or the same
/// query answers differently depending on how it is consumed.
struct LogicalFilteredEntityIdMaterializer<'a> {
    query: &'a PyQueryIter,
}

impl<Context> RowMaterializer<Context> for LogicalFilteredEntityIdMaterializer<'_> {
    type Output = Entity;
    type Error = PyErr;

    fn matches(&self, entity: &FilteredEntityAccess<'_, '_>) -> bool {
        self.query.matches_logical_types(entity)
    }

    fn materialize(
        &self,
        entity: &mut FilteredEntityAccess<'_, '_>,
        _context: Context,
    ) -> PyResult<Entity> {
        Ok(entity.id())
    }
}

impl PyQueryIter {
    fn register_ad_hoc_custom_type(
        world: &mut World,
        custom_component_ids: &mut HashMap<*const PyTypeObject, ComponentId>,
        component_type: &PyComponentType,
        py: Python,
    ) {
        if let PyComponentType::Custom(type_ptr) = component_type {
            custom_component_ids
                .entry(*type_ptr)
                .or_insert_with(|| component_type.register_simple(world, py));
        }
    }

    /// # Safety
    /// `validity` must be invalidated before `world` is destroyed or made
    /// inaccessible to the caller.
    pub unsafe fn from_world(
        world: &mut World,
        param: PyQueryParam,
        validity: ValidityFlag,
        py: Python,
    ) -> Self {
        let mut custom_component_ids = HashMap::new();
        for data in &param.data {
            for ty in data.component_types() {
                Self::register_ad_hoc_custom_type(world, &mut custom_component_ids, &ty, py);
            }
        }
        for filter in &param.filters {
            match filter {
                QueryFilter::With(filter) => {
                    for ty in &filter.values {
                        Self::register_ad_hoc_custom_type(world, &mut custom_component_ids, ty, py);
                    }
                }
                QueryFilter::Without(filter) => {
                    for ty in &filter.values {
                        Self::register_ad_hoc_custom_type(world, &mut custom_component_ids, ty, py);
                    }
                }
                QueryFilter::Changed(filter) => Self::register_ad_hoc_custom_type(
                    world,
                    &mut custom_component_ids,
                    &filter.component_type,
                    py,
                ),
                QueryFilter::Added(filter) => Self::register_ad_hoc_custom_type(
                    world,
                    &mut custom_component_ids,
                    &filter.component_type,
                    py,
                ),
                QueryFilter::Or(filter) => {
                    for ty in filter.values.iter().flat_map(QueryFilter::component_types) {
                        Self::register_ad_hoc_custom_type(
                            world,
                            &mut custom_component_ids,
                            &ty,
                            py,
                        );
                    }
                }
            }
        }

        let cached = Box::new(CachedQuery::build(
            world,
            Arc::new(param),
            Arc::new(custom_component_ids),
            py,
        ));
        let world_cell = world.as_unsafe_world_cell();
        // SAFETY: `cached` was just built from this same `world`, and `validity`
        // is the caller's fence for that World.
        unsafe { Self::new_owned(cached, world_cell, validity) }
    }

    /// Creates a new runtime query bound to a cached query state and world cell.
    ///
    /// The heavy per-system work (component-id registration, QueryState
    /// construction) lives in `cached`, built once in `DynamicSystem::initialize`.
    ///
    /// SAFETY: `cached` must remain valid for the lifetime of this object (it is
    /// owned by the DynamicSystem, which outlives every run), and `world_cell` must
    /// reference the World that `cached` was built from. Both are fenced by
    /// `validity`, which is invalidated when the system finishes.
    pub unsafe fn new(
        cached: &CachedQuery,
        world_cell: UnsafeWorldCell,
        validity: ValidityFlag,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        // SAFETY: this constructor carries the same stable-cache, matching-world,
        // and validity-window contract as QueryRuntimeCore::new.
        let runtime = unsafe {
            QueryRuntimeCore::new(Some(&cached.core), world_cell, validity, last_run, this_run)
        };

        Self {
            // The cached system/observer parameter owns the classes for this
            // validity window. Avoid a per-run Python incref here; an escaped
            // runtime is invalid before the cache can disappear.
            param: Arc::new(cached.param.clone_without_retained_types()),
            cached: QueryCache::Borrowed(NonNull::from(cached)),
            runtime,
            values_buffer: RefCell::new(SmallVec::new()),
            layout_cache: RefCell::new(HashMap::new()),
        }
    }

    /// # Safety
    /// `world_cell` must reference the World used to build `cached`, and
    /// `validity` must be invalidated before that World is destroyed.
    pub unsafe fn new_owned(
        mut cached: Box<CachedQuery>,
        world_cell: UnsafeWorldCell,
        validity: ValidityFlag,
    ) -> Self {
        // SAFETY: this constructor carries the caller's matching-world and
        // validity-window contract; the Box keeps `cached.core` pinned for as
        // long as the runtime can reach it.
        let runtime =
            unsafe { QueryRuntimeCore::new_live(Some(&cached.core), world_cell, validity) };

        let param = Arc::new(cached.param.as_ref().clone());
        cached.param = Arc::new(cached.param.clone_without_retained_types());

        Self {
            param,
            cached: QueryCache::Owned(cached),
            runtime,
            values_buffer: RefCell::new(SmallVec::new()),
            layout_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Borrow the static cached query state.
    ///
    /// SAFETY: `cached` points into a CachedQuery owned by the DynamicSystem, which
    /// outlives every run; access is fenced by the same ValidityFlag as `world_cell`.
    #[inline]
    fn cached(&self) -> &CachedQuery {
        self.cached.get()
    }

    fn is_ad_hoc(&self) -> bool {
        self.cached.is_owned()
    }

    fn snapshot_entity_ids(&self) -> PyResult<Vec<Entity>> {
        let mut token = self
            .runtime
            .begin_iteration()
            .map_err(query_runtime_error_to_py)?;
        let mut entities = Vec::new();
        while let Some(entity) = self
            .runtime
            .advance_with(
                &mut token,
                &LogicalFilteredEntityIdMaterializer { query: self },
                (),
            )
            .map_err(query_execution_error_to_py)?
        {
            entities.push(entity);
        }
        Ok(entities)
    }

    fn extract_ad_hoc_components(&self, entity: Entity, py: Python) -> PyResult<()> {
        let world_ptr = unsafe { self.runtime.world_ptr() }.map_err(query_runtime_error_to_py)?;
        let validity = self.runtime.validity().clone();
        // SAFETY: the runtime validity fence protects world_ptr. Ad-hoc callers
        // reach this only after the temporary QueryState entity access or iterator
        // has been released.
        let world = unsafe { PyWorld::new(&mut *world_ptr, validity) };
        let mut values_buffer = self.values_buffer.borrow_mut();
        values_buffer.clear();

        for param_type in &self.param.data {
            match param_type {
                QueryData::Entity => {
                    values_buffer.push(Py::new(py, PyEntity(entity))?.into_any());
                }
                QueryData::Has { ty } => {
                    let component_id = self.component_id(ty);
                    // SAFETY: the validity fence above protects this shared World access.
                    let world = unsafe { &*world_ptr };
                    let has_component = world
                        .get_entity(entity)
                        .is_ok_and(|entity_ref| entity_ref.contains_id(component_id));
                    values_buffer.push(has_component.into_py_any(py)?);
                }
                QueryData::AnyOf { items } => {
                    let mut values = Vec::with_capacity(items.len());
                    for item in items {
                        // SAFETY: the runtime validity fence protects this
                        // shared World access for the logical identity check.
                        let world_ref = unsafe { &*world_ptr };
                        let logical_mismatch = item.logical_type_id.is_some_and(|logical_id| {
                            !entity_logical_type_matches(world_ref, entity, item.ty, logical_id)
                        });
                        if logical_mismatch {
                            values.push(py.None());
                            continue;
                        }
                        let type_ptr = match item.ty {
                            PyComponentType::Dynamic(type_ptr)
                            | PyComponentType::Resource(type_ptr)
                            | PyComponentType::Custom(type_ptr) => type_ptr,
                        };
                        // SAFETY: the query parameter retains every custom
                        // class and native registry type objects are stable.
                        let component_type = unsafe {
                            Bound::from_borrowed_ptr(
                                py,
                                type_ptr.cast_mut().cast::<pyo3::ffi::PyObject>(),
                            )
                        };
                        let value = if item.mutable {
                            world.get_mut(py, &PyEntity(entity), component_type)?
                        } else {
                            world.get(py, &PyEntity(entity), component_type)?
                        };
                        values.push(value.unwrap_or_else(|| py.None()));
                    }
                    values_buffer.push(PyTuple::new(py, values)?.into_any().unbind());
                }
                QueryData::Component {
                    ty,
                    mutable,
                    optional,
                    logical_type_id,
                } => {
                    // An optional param whose logical specialization does not
                    // match materializes as None, matching the system-query
                    // path. A required mismatch was already excluded from the
                    // id snapshot.
                    if let Some(logical_type_id) = logical_type_id {
                        // SAFETY: same validity fence as `world_ptr` above; the
                        // shared reference does not outlive this check.
                        let world_ref = unsafe { &*world_ptr };
                        if !entity_logical_type_matches(world_ref, entity, *ty, *logical_type_id) {
                            if *optional {
                                values_buffer.push(py.None());
                                continue;
                            }
                            return Err(PyRuntimeError::new_err(
                                "Query component was removed before row materialization",
                            ));
                        }
                    }
                    let type_ptr = match ty {
                        PyComponentType::Dynamic(type_ptr)
                        | PyComponentType::Resource(type_ptr)
                        | PyComponentType::Custom(type_ptr) => *type_ptr,
                    };
                    // SAFETY: registered component type objects remain live for the
                    // interpreter lifetime and the borrowed Bound does not escape.
                    let component_type = unsafe {
                        Bound::from_borrowed_ptr(
                            py,
                            type_ptr.cast_mut().cast::<pyo3::ffi::PyObject>(),
                        )
                    };
                    let value = if *mutable {
                        world.get_mut(py, &PyEntity(entity), component_type)?
                    } else {
                        world.get(py, &PyEntity(entity), component_type)?
                    };
                    match value {
                        Some(value) => values_buffer.push(value),
                        None if *optional => values_buffer.push(py.None()),
                        None => {
                            return Err(PyRuntimeError::new_err(
                                "Query component was removed before row materialization",
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn materialize_ad_hoc_entity(&self, entity: Entity, py: Python) -> PyResult<Py<PyAny>> {
        self.extract_ad_hoc_components(entity, py)?;
        self.materialized_result(py)
    }

    /// Get the cached extraction function for a dynamic component type.
    #[inline(always)]
    pub(crate) fn get_extract_fn(&self, ty: &PyComponentType) -> Option<ExtractFn> {
        self.cached().extract_fns.get(ty).copied()
    }

    fn matches_logical_types(&self, entity: &FilteredEntityAccess<'_, '_>) -> bool {
        let Some(map_component_id) = self.cached().logical_type_map_component_id else {
            return true;
        };
        let map = entity.get_by_id(map_component_id).map(|pointer| {
            // SAFETY: CachedQuery registered `LogicalTypeMap` for this exact
            // component ID and declared read access to it in the QueryState.
            unsafe { &*(pointer.as_ptr() as *const LogicalTypeMap) }
        });
        self.param.data.iter().all(|data| match data {
            QueryData::Component {
                ty,
                optional,
                logical_type_id: Some(logical_type_id),
                ..
            } => {
                // Optional query data never filters out the entity. A missing
                // native component or a different logical specialization is
                // materialized as None below.
                *optional
                    || ty.type_id().is_some_and(|native_type| {
                        map.is_some_and(|map| map.matches(native_type, *logical_type_id))
                    })
            }
            QueryData::AnyOf { items } => items.iter().any(|item| {
                entity.get_by_id(self.component_id(&item.ty)).is_some()
                    && item.logical_type_id.is_none_or(|logical_id| {
                        self.matches_logical_component(entity, &item.ty, logical_id)
                    })
            }),
            _ => true,
        })
    }

    #[inline(always)]
    fn component_id(&self, ty: &PyComponentType) -> ComponentId {
        match ty {
            PyComponentType::Custom(type_ptr) => *self
                .cached()
                .custom_component_ids
                .get(type_ptr)
                .expect("Custom component ID should be registered"),
            _ => *self
                .cached()
                .component_id_cache
                .get(ty)
                .expect("Component ID should be cached"),
        }
    }

    fn matches_logical_component(
        &self,
        entity: &FilteredEntityAccess<'_, '_>,
        ty: &PyComponentType,
        logical_type_id: LogicalTypeId,
    ) -> bool {
        let Some(map_component_id) = self.cached().logical_type_map_component_id else {
            return false;
        };
        let Some(native_type) = ty.type_id() else {
            return false;
        };
        entity.get_by_id(map_component_id).is_some_and(|pointer| {
            // SAFETY: CachedQuery registered `LogicalTypeMap` for this exact
            // component ID and declared read access to it in the QueryState.
            let map = unsafe { &*(pointer.as_ptr() as *const LogicalTypeMap) };
            map.matches(native_type, logical_type_id)
        })
    }

    /// Extract a custom component from an entity and return as PyObject
    ///
    /// Handles both wrapper storage (primitives with lazy proxy) and PyObject storage.
    /// This consolidates the 117 lines of custom component logic repeated 3 times
    /// (in extract_components_from_entity, single, and get methods).
    ///
    /// Called by PyComponentType::extract_from_entity() macro dispatch method.
    pub(crate) fn extract_custom_component(
        &self,
        type_ptr: *const PyTypeObject,
        entity: &mut FilteredEntityAccess,
        component_id: ComponentId,
        access_mode: AccessMode,
        py: Python,
    ) -> PyResult<Py<PyAny>> {
        // Get cached storage type + layout, or compute and cache on first access
        let (storage_type, cached_layout) = {
            let cache = self.layout_cache.borrow();
            if let Some(cached) = cache.get(&type_ptr) {
                (cached.0, cached.1.clone())
            } else {
                drop(cache);
                // SAFETY: type_ptr is valid for the lifetime of the Python interpreter
                let py_type = unsafe {
                    pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject)
                };
                let st = if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                    ComponentStorageType::from_python_class(cls)
                        .unwrap_or(ComponentStorageType::PyObject)
                } else {
                    ComponentStorageType::PyObject
                };
                let layout = if let ComponentStorageType::Wrapper(_) = &st {
                    let cls = py_type
                        .cast::<pyo3::types::PyType>()
                        .expect("Type pointer should be valid");
                    Some(Arc::new(
                        crate::ecs::component_layout::ComponentLayout::from_annotations(cls)
                            .expect("Layout should be computable for wrapper components"),
                    ))
                } else {
                    None
                };
                self.layout_cache
                    .borrow_mut()
                    .insert(type_ptr, (st, layout.clone()));
                (st, layout)
            }
        };

        match storage_type {
            ComponentStorageType::Wrapper(wrapper_size) => {
                let data_ptr = if access_mode == AccessMode::Write {
                    let base = entity
                        .get_mut_ptr_by_id_unchanged(component_id)
                        .expect("Custom component should exist on matched entity");
                    // SAFETY: mutable query access produced `base` for the wrapper
                    // descriptor registered as `wrapper_size`.
                    unsafe { wrapper_size.get_data_ptr_from_base(base) }
                } else {
                    let untyped = entity
                        .get_by_id(component_id)
                        .expect("Custom component should exist on matched entity");
                    // SAFETY: the read-only proxy never writes through this pointer;
                    // the entity matched the registered wrapper descriptor.
                    unsafe { wrapper_size.get_ref_ptr_as_mut(untyped) }
                };

                let layout = cached_layout.expect("Wrapper storage must have layout");

                // Create lazy wrapper proxy
                let entity_id = entity.id();
                let validity = self.runtime.validity().with_access_mode(access_mode);
                let mutable = access_mode == AccessMode::Write;
                // SAFETY: this proxy shares the query runtime's validity
                // window and may touch only its declared component.
                let world_cell =
                    unsafe { self.runtime.world_cell() }.map_err(query_runtime_error_to_py)?;
                // SAFETY: `data_ptr` addresses this entity's wrapper bytes for
                // `component_id`, `layout` is the descriptor it was registered
                // with, and the proxy is fenced by the same validity window.
                let proxy = unsafe {
                    PyLazyWrapperProxy::new(
                        data_ptr,
                        layout,
                        type_ptr,
                        validity,
                        mutable, // true for Mut[T], false for read-only
                        component_id,
                        entity_id,
                        std::ptr::null_mut(),
                        world_cell,
                        Some(self.runtime.run_ticks()),
                        // Query iteration: the cached data_ptr is kept valid by the
                        // ValidityFlag for the duration of the access (fast path).
                        ProxyKind::QueryItem,
                    )
                };

                let py_obj = Py::new(py, proxy).expect("Failed to create lazy wrapper proxy");
                Ok(py_obj.into_any())
            }
            ComponentStorageType::PyObject => {
                // PyObject storage - return borrowed reference to ECS-stored Python object
                use crate::ecs::custom_component::PyCustomComponent;

                let entity_id = entity.id();

                // Get pointer to the PyAny in ECS storage
                // SAFETY: We know this is a Py<PyAny> because that's how we registered it
                // NOTE: We use get_by_id() for both mutable and immutable access.
                // Change detection is handled by __setattr__ hook + stored entity context.
                let untyped_ptr = entity
                    .get_by_id(component_id)
                    .expect("Custom component should exist on matched entity")
                    .as_ptr();

                let py_object = unsafe {
                    let py_any_ref = &*(untyped_ptr as *const Py<PyAny>);
                    py_any_ref.clone_ref(py)
                };

                // Create borrowed reference with validity tracking and entity context
                let validity = self.runtime.validity().with_access_mode(access_mode);
                // SAFETY: this wrapper shares the query runtime's validity
                // window and declared component access.
                let world_cell =
                    unsafe { self.runtime.world_cell() }.map_err(query_runtime_error_to_py)?;

                let custom_comp = PyCustomComponent::from_object(
                    py_object,
                    validity,
                    component_id,
                    entity_id,
                    world_cell,
                    Some(self.runtime.run_ticks()),
                );

                let py_obj = Py::new(py, (custom_comp, crate::ecs::component::PyComponent))
                    .expect("Failed to create PyCustomComponent");
                Ok(py_obj.into_any())
            }
        }
    }

    fn extract_query_component(
        &self,
        entity: &mut FilteredEntityAccess,
        ty: PyComponentType,
        mutable: bool,
        logical_type_id: Option<LogicalTypeId>,
        py: Python<'_>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let component_id = self.component_id(&ty);
        let native_missing = entity.get_by_id(component_id).is_none();
        let logical_mismatch = logical_type_id
            .is_some_and(|logical_id| !self.matches_logical_component(entity, &ty, logical_id));
        if native_missing || logical_mismatch {
            return Ok(None);
        }

        let access_mode = if mutable {
            AccessMode::Write
        } else {
            AccessMode::Read
        };
        let mut validity = self.runtime.validity().with_access_mode(access_mode);
        if mutable {
            let ticks = self.runtime.run_ticks();
            // SAFETY: the query declared mutable access to this exact
            // component, and the context is fenced by `validity`.
            let context = unsafe {
                ComponentWriteContext::new(
                    self.runtime
                        .world_cell()
                        .map_err(query_runtime_error_to_py)?,
                    entity.id(),
                    component_id,
                    ticks.last_run,
                    ticks.this_run,
                )
            };
            validity = validity.with_component_write_context(context);
        }

        let mut value = ty.extract_from_entity(entity, component_id, validity, py, self)?;
        if let Some(logical_type_id) = logical_type_id {
            value = value
                .bind(py)
                .call_method1("_materialize_logical_type_id", (logical_type_id.get(),))?
                .unbind();
        }
        Ok(Some(value))
    }

    /// Extract component data from an entity and populate values_buffer
    ///
    /// This helper consolidates the component extraction logic shared by
    /// __next__(), single(), and get() methods.
    fn extract_components_from_entity(
        &self,
        entity: &mut FilteredEntityAccess,
        py: Python,
    ) -> PyResult<()> {
        let mut values_buffer = self.values_buffer.borrow_mut();
        values_buffer.clear();

        for param_type in &self.param.data {
            match param_type {
                QueryData::Entity => {
                    let py_entity = PyEntity(entity.id());
                    let obj = Py::new(py, py_entity).expect("Failed to create PyEntity");
                    values_buffer.push(obj.into_any());
                }
                QueryData::Has { ty } => {
                    values_buffer.push(entity.contains_id(self.component_id(ty)).into_py_any(py)?);
                }
                QueryData::AnyOf { items } => {
                    let mut values = Vec::with_capacity(items.len());
                    for item in items {
                        values.push(
                            self.extract_query_component(
                                entity,
                                item.ty,
                                item.mutable,
                                item.logical_type_id,
                                py,
                            )?
                            .unwrap_or_else(|| py.None()),
                        );
                    }
                    values_buffer.push(PyTuple::new(py, values)?.into_any().unbind());
                }
                QueryData::Component {
                    ty,
                    mutable,
                    optional,
                    logical_type_id,
                } => {
                    let value =
                        self.extract_query_component(entity, *ty, *mutable, *logical_type_id, py)?;
                    match value {
                        Some(value) => values_buffer.push(value),
                        None if *optional => values_buffer.push(py.None()),
                        None => {
                            return Err(PyRuntimeError::new_err(
                                "Query component was removed before row materialization",
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn materialized_result(&self, py: Python) -> PyResult<Py<PyAny>> {
        let values_buffer = self.values_buffer.borrow();
        if self.param.single {
            Ok(values_buffer[0].clone_ref(py))
        } else {
            let tuple = PyTuple::new(py, values_buffer.iter())?;
            Ok(tuple.into_any().unbind())
        }
    }

    pub(crate) fn materialize_single(
        &self,
        py: Python,
    ) -> Result<Py<PyAny>, QueryExecutionError<PyErr>> {
        if self.is_ad_hoc() {
            let entity = self
                .runtime
                .single_with(&LogicalFilteredEntityIdMaterializer { query: self }, ())?;
            return self
                .materialize_ad_hoc_entity(entity, py)
                .map_err(QueryExecutionError::Materialize);
        }
        let materializer = PyQueryRowMaterializer { query: self };
        self.runtime.single_with(&materializer, py)
    }
}

/// Private row adapter. Keeping the public neutral trait off `PyQueryIter`
/// ensures row materialization cannot bypass PyO3's exclusive object borrow.
struct PyQueryRowMaterializer<'a> {
    query: &'a PyQueryIter,
}

impl<'py> RowMaterializer<Python<'py>> for PyQueryRowMaterializer<'_> {
    type Output = Py<PyAny>;
    type Error = PyErr;

    fn matches(&self, entity: &FilteredEntityAccess<'_, '_>) -> bool {
        self.query.matches_logical_types(entity)
    }

    fn materialize(
        &self,
        entity: &mut FilteredEntityAccess<'_, '_>,
        py: Python<'py>,
    ) -> PyResult<Self::Output> {
        self.query.extract_components_from_entity(entity, py)?;
        self.query.materialized_result(py)
    }
}

#[pymethods]
impl PyQueryIter {
    /// Report independently retained classes and materialized rows still cached
    /// by this runtime wrapper.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.param.retained_types {
            visit.call(class.as_ref())?;
        }
        let Ok(values) = self.values_buffer.try_borrow() else {
            return Ok(());
        };
        for value in values.iter() {
            visit.call(value)?;
        }
        Ok(())
    }

    /// Makes this object iterable.
    /// Resets the iterator state so that re-iteration works correctly
    /// (matching Bevy's query semantics where each .iter() call is fresh).
    /// Rejects nested iteration (matching Bevy's borrow-checker prevention).
    fn __iter__(slf: Py<Self>, py: Python) -> PyResult<Py<PyQueryIterator>> {
        let state = {
            let query = slf.borrow(py);
            if query.is_ad_hoc() {
                QueryIteratorState::Snapshot(query.snapshot_entity_ids()?.into_iter())
            } else {
                QueryIteratorState::Runtime(
                    query
                        .runtime
                        .begin_iteration()
                        .map_err(query_runtime_error_to_py)?,
                )
            }
        };
        Py::new(
            py,
            PyQueryIterator {
                query: slf.clone_ref(py),
                state,
            },
        )
    }

    /// Returns the number of entities matching the query.
    ///
    /// **Warning: O(n)** - this iterates all matching entities to count them.
    /// Python users calling `len(query)` may expect O(1) but Bevy's
    /// `QueryState` does not cache entity counts.
    fn __len__(&mut self) -> PyResult<usize> {
        if self.runtime.has_unadvanced_iterator() {
            // CPython asks the original iterable for a length hint after
            // `list(query)` has already called `iter(query)`. TypeError means
            // "no hint" to list(), while direct len(query) remains rejected.
            return Err(PyTypeError::new_err(
                "Query length is unavailable while an iterator is pending",
            ));
        }
        if self.cached().logical_type_map_component_id.is_none() {
            self.runtime.count().map_err(query_runtime_error_to_py)
        } else {
            let materializer = PyQueryRowMaterializer { query: self };
            self.runtime
                .count_with::<Python<'_>, _>(&materializer)
                .map_err(query_runtime_error_to_py)
        }
    }

    /// Get exactly one entity from the query.
    /// Returns an error if there are 0 or 2+ entities matching the query.
    fn single(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        self.materialize_single(py)
            .map_err(query_execution_error_to_py)
    }

    /// Check if the query has no matching entities.
    /// Returns true if there are no entities matching the query filters.
    fn is_empty(&mut self) -> PyResult<bool> {
        if self.cached().logical_type_map_component_id.is_none() {
            self.runtime.is_empty().map_err(query_runtime_error_to_py)
        } else {
            let materializer = PyQueryRowMaterializer { query: self };
            self.runtime
                .is_empty_with::<Python<'_>, _>(&materializer)
                .map_err(query_runtime_error_to_py)
        }
    }

    /// Get components for a specific entity by ID.
    /// Returns None if the entity doesn't match the query filters.
    /// Returns an error if the entity doesn't have the queried components.
    fn get(&mut self, entity: PyEntity, py: Python) -> PyResult<Option<Py<PyAny>>> {
        if self.is_ad_hoc() {
            let matched = self
                .runtime
                .get_with(
                    entity.0,
                    &LogicalFilteredEntityIdMaterializer { query: self },
                    (),
                )
                .map_err(query_execution_error_to_py)?;
            return matched
                .map(|entity| self.materialize_ad_hoc_entity(entity, py))
                .transpose();
        }
        let materializer = PyQueryRowMaterializer { query: self };
        self.runtime
            .get_with(entity.0, &materializer, py)
            .map_err(query_execution_error_to_py)
    }

    /// Iterate over query results for a specific list of entities.
    /// Entities that don't match the query filters are skipped.
    ///
    /// # Arguments
    /// * `entities` - An iterable of Entity objects to query
    ///
    /// # Returns
    /// A list of query results in the same order as the input entities (skipping non-matching ones)
    ///
    /// # Example
    /// ```python
    /// entities = [entity1, entity2, entity3]
    /// results = query.iter_many(entities)
    /// for result in results:
    ///     # Process each matching entity's components
    ///     pass
    /// ```
    fn iter_many(&mut self, entities: &Bound<'_, PyAny>, py: Python) -> PyResult<Vec<Py<PyAny>>> {
        // Preserve the validity contract even for an empty input iterable.
        self.runtime
            .check_valid()
            .map_err(query_runtime_error_to_py)?;
        let mut results = Vec::new();

        for entity_obj in entities.try_iter()? {
            let entity_id: PyEntity = entity_obj?.extract()?;
            if self.is_ad_hoc() {
                if let Some(entity) = self
                    .runtime
                    .get_with(
                        entity_id.0,
                        &LogicalFilteredEntityIdMaterializer { query: self },
                        (),
                    )
                    .map_err(query_execution_error_to_py)?
                {
                    results.push(self.materialize_ad_hoc_entity(entity, py)?);
                }
            } else {
                let materializer = PyQueryRowMaterializer { query: self };
                if let Some(row) = self
                    .runtime
                    .get_with(entity_id.0, &materializer, py)
                    .map_err(query_execution_error_to_py)?
                {
                    results.push(row);
                }
            }
        }

        Ok(results)
    }
}

#[pymethods]
impl PyQueryIterator {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.query)
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        let query = self.query.borrow(py);
        match &mut self.state {
            QueryIteratorState::Runtime(token) => {
                let materializer = PyQueryRowMaterializer { query: &query };
                match query
                    .runtime
                    .advance_with(token, &materializer, py)
                    .map_err(query_execution_error_to_py)?
                {
                    Some(row) => Ok(row),
                    None => Err(PyStopIteration::new_err("")),
                }
            }
            QueryIteratorState::Snapshot(entities) => match entities.next() {
                Some(entity) => query.materialize_ad_hoc_entity(entity, py),
                None => Err(PyStopIteration::new_err("")),
            },
        }
    }
}
