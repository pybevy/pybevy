use std::{cell::RefCell, collections::HashMap, ptr::NonNull, sync::Arc};

use bevy::{
    ecs::{
        change_detection::{ComponentTicks, Tick},
        component::ComponentId,
        query::{QueryIter, QueryState},
        world::{FilteredEntityMut, FilteredEntityRef, unsafe_world_cell::UnsafeWorldCell},
    },
    prelude::*,
};
use pybevy_core::{ExtractFn, FilteredEntityAccess, registry::global_registry};
use pybevy_ecs::shared::query_builder_ext::{
    QueryBuildSpec, QueryComponent, build_query_state, build_query_state_ref,
};
use pyo3::{
    exceptions::{PyRuntimeError, PyStopIteration},
    ffi::PyTypeObject,
    prelude::*,
    types::PyTuple,
};
use smallvec::SmallVec;

use crate::ecs::{
    PyEntity,
    component_layout::ComponentStorageType,
    component_type::{PyComponentType, register_component_id},
    filter::QueryFilter,
    helpers::validity_guard::{AccessMode, ValidityFlag},
    lazy_wrapper_proxy::PyLazyWrapperProxy,
    query::query_param::{PyQueryParam, QueryData},
};

/// Type-erased Bevy QueryState. Owns a heap-allocated QueryState behind a raw pointer.
///
/// SAFETY: The caller must guarantee the erased lifetimes (tied to system execution scope).
/// Drop reconstructs the correct Box type to deallocate.
///
/// Methods that produce borrows (`create_iter`, `get_entity`) are intentionally designed
/// to not borrow `self`, so that PyQueryIter can call them without conflicting with
/// borrows of other fields (e.g. `extract_components_from_entity`).
enum ErasedQueryState {
    /// `QueryState<FilteredEntityRef>` - all components read-only.
    ReadOnly(*mut ()),
    /// `QueryState<FilteredEntityMut>` - at least one `Mut[T]` component.
    Mutable(*mut ()),
}

impl ErasedQueryState {
    fn from_ref(qs: QueryState<FilteredEntityRef>) -> Self {
        Self::ReadOnly(Box::into_raw(Box::new(qs)) as *mut ())
    }

    fn from_mut(qs: QueryState<FilteredEntityMut>) -> Self {
        Self::Mutable(Box::into_raw(Box::new(qs)) as *mut ())
    }

    /// Returns (is_read_only, raw_pointer) for use in methods that need to avoid
    /// borrowing self (to prevent conflicts with other field borrows on PyQueryIter).
    fn parts(&self) -> (bool, *mut ()) {
        match self {
            Self::ReadOnly(p) => (true, *p),
            Self::Mutable(p) => (false, *p),
        }
    }

    /// Count matching entities (O(n) - iterates all).
    ///
    /// SAFETY: declared access from `initialize` covers this state's access and the
    /// executor prevents conflicting systems from running concurrently, so the
    /// unchecked query has unique access to the components it reads.
    fn count(&self, cell: UnsafeWorldCell, last_run: Tick, this_run: Tick) -> usize {
        let (read_only, p) = self.parts();
        unsafe {
            if read_only {
                let qs = &mut *(p as *mut QueryState<FilteredEntityRef>);
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .iter_inner()
                    .count()
            } else {
                let qs = &mut *(p as *mut QueryState<FilteredEntityMut>);
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .iter_inner()
                    .count()
            }
        }
    }

    /// Check if no entities match.
    ///
    /// SAFETY: declared access from `initialize` covers this state's access and the
    /// executor prevents conflicting systems from running concurrently, so the
    /// unchecked query has unique access to the components it reads.
    fn is_empty_check(&self, cell: UnsafeWorldCell, last_run: Tick, this_run: Tick) -> bool {
        let (read_only, p) = self.parts();
        unsafe {
            if read_only {
                let qs = &mut *(p as *mut QueryState<FilteredEntityRef>);
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .is_empty()
            } else {
                let qs = &mut *(p as *mut QueryState<FilteredEntityMut>);
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .is_empty()
            }
        }
    }
}

/// Create a new lazy iterator. Freestanding to avoid borrowing ErasedQueryState.
///
/// Uses `query_unchecked_with_ticks`, which calls `update_archetypes_unsafe_world_cell`
/// internally, so the cached state picks up archetypes created since `initialize`.
/// Ticks flow explicitly so per-entity Changed/Added checks stay consistent.
///
/// SAFETY: `qs_ptr` must point to a valid QueryState of the type indicated by
/// `read_only`. `cell` must point to the World this state was initialized from.
/// The declared access from `initialize` covers this state and the executor
/// prevents conflicting systems from running concurrently, so the unchecked
/// query has unique access to the components it touches.
unsafe fn erased_create_iter(
    read_only: bool,
    qs_ptr: *mut (),
    cell: UnsafeWorldCell,
    last_run: Tick,
    this_run: Tick,
) -> ErasedQueryIter {
    if read_only {
        let qs = unsafe { &mut *(qs_ptr as *mut QueryState<FilteredEntityRef>) };
        let iter = unsafe { qs.query_unchecked_with_ticks(cell, last_run, this_run) }.iter_inner();
        ErasedQueryIter::ReadOnly(Box::into_raw(Box::new(iter)) as *mut ())
    } else {
        let qs = unsafe { &mut *(qs_ptr as *mut QueryState<FilteredEntityMut>) };
        let iter = unsafe { qs.query_unchecked_with_ticks(cell, last_run, this_run) }.iter_inner();
        ErasedQueryIter::Mutable(Box::into_raw(Box::new(iter)) as *mut ())
    }
}

/// Look up a single entity by ID. Freestanding to avoid borrowing ErasedQueryState.
///
/// SAFETY: `qs_ptr` must point to a valid QueryState of the type indicated by
/// `read_only`. `cell` must point to the World this state was initialized from.
/// The declared access from `initialize` covers this state and the executor
/// prevents conflicting systems from running concurrently, so the unchecked
/// query has unique access to the components it touches.
unsafe fn erased_get_entity<'a>(
    read_only: bool,
    qs_ptr: *mut (),
    cell: UnsafeWorldCell<'a>,
    last_run: Tick,
    this_run: Tick,
    entity: Entity,
) -> Option<FilteredEntityAccess<'a, 'a>> {
    if read_only {
        let qs = unsafe { &mut *(qs_ptr as *mut QueryState<FilteredEntityRef>) };
        unsafe { qs.query_unchecked_with_ticks(cell, last_run, this_run) }
            .get_inner(entity)
            .ok()
            .map(FilteredEntityAccess::Ref)
    } else {
        let qs = unsafe { &mut *(qs_ptr as *mut QueryState<FilteredEntityMut>) };
        unsafe { qs.query_unchecked_with_ticks(cell, last_run, this_run) }
            .get_inner(entity)
            .ok()
            .map(FilteredEntityAccess::Mut)
    }
}

impl Drop for ErasedQueryState {
    fn drop(&mut self) {
        let p = match self {
            Self::ReadOnly(p) | Self::Mutable(p) => *p,
        };
        if p.is_null() {
            return;
        }
        unsafe {
            match self {
                Self::ReadOnly(_) => {
                    let _ = Box::from_raw(p as *mut QueryState<FilteredEntityRef>);
                }
                Self::Mutable(_) => {
                    let _ = Box::from_raw(p as *mut QueryState<FilteredEntityMut>);
                }
            }
        }
    }
}

/// Type-erased Bevy QueryIter. Owns a heap-allocated iterator behind a raw pointer.
///
/// SAFETY: The erased lifetimes are tied to the system execution scope.
enum ErasedQueryIter {
    ReadOnly(*mut ()),
    Mutable(*mut ()),
}

impl ErasedQueryIter {
    /// Advance the iterator, returning the next entity wrapped in `FilteredEntityAccess`.
    fn next(&mut self) -> Option<FilteredEntityAccess<'_, '_>> {
        unsafe {
            match self {
                Self::ReadOnly(p) => {
                    let iter = &mut *(*p as *mut QueryIter<FilteredEntityRef, ()>);
                    iter.next().map(FilteredEntityAccess::Ref)
                }
                Self::Mutable(p) => {
                    let iter = &mut *(*p as *mut QueryIter<FilteredEntityMut, ()>);
                    iter.next().map(FilteredEntityAccess::Mut)
                }
            }
        }
    }
}

impl Drop for ErasedQueryIter {
    fn drop(&mut self) {
        let p = match self {
            Self::ReadOnly(p) | Self::Mutable(p) => *p,
        };
        if p.is_null() {
            return;
        }
        unsafe {
            match self {
                Self::ReadOnly(_) => {
                    let _ = Box::from_raw(p as *mut QueryIter<FilteredEntityRef, ()>);
                }
                Self::Mutable(_) => {
                    let _ = Box::from_raw(p as *mut QueryIter<FilteredEntityMut, ()>);
                }
            }
        }
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

    /// The Bevy QueryState (type-erased, owns the heap allocation).
    query_state: ErasedQueryState,

    /// Maps PyComponentType to their registered ComponentIds (cached for fast access).
    component_id_cache: HashMap<PyComponentType, ComponentId>,

    /// Maps custom component type pointers to their registered ComponentIds (shared via Arc).
    custom_component_ids: Arc<HashMap<*const PyTypeObject, ComponentId>>,

    /// Per-parameter access modes (Read or Write), indexed by parameter position.
    param_access_modes: SmallVec<[AccessMode; 8]>,

    /// Extraction function pointers for Dynamic components, indexed by parameter position.
    extract_fns: SmallVec<[Option<ExtractFn>; 8]>,

    /// ComponentIds for Changed[T] tick filters - entities must pass per-entity tick check.
    changed_filter_ids: Vec<ComponentId>,
    /// ComponentIds for Added[T] tick filters - entities must pass per-entity tick check.
    added_filter_ids: Vec<ComponentId>,

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
    ) -> Self {
        // Collect and register all component IDs (tracking optional and mutable status)
        let mut component_ids = Vec::new();
        for param_type in &param.data {
            if let QueryData::Component {
                ty: comp_type,
                optional,
                mutable,
                ..
            } = param_type
            {
                let id = register_component_id(world, comp_type, &custom_component_ids);
                component_ids.push(QueryComponent {
                    id,
                    optional: *optional,
                    mutable: *mutable,
                });
            }
        }

        // Collect filter component IDs
        let mut with_filter_ids = Vec::new();
        let mut without_filter_ids = Vec::new();
        let mut changed_filter_ids = Vec::new();
        let mut added_filter_ids = Vec::new();
        let mut anyof_filter_ids = Vec::new();

        for filter in &param.filters {
            match filter {
                QueryFilter::With(with) => {
                    for comp_type in &with.values {
                        let id = register_component_id(world, comp_type, &custom_component_ids);
                        with_filter_ids.push(id);
                    }
                }
                QueryFilter::Without(without) => {
                    for comp_type in &without.values {
                        let id = register_component_id(world, comp_type, &custom_component_ids);
                        without_filter_ids.push(id);
                    }
                }
                QueryFilter::Changed(changed) => {
                    let id = register_component_id(
                        world,
                        &changed.component_type,
                        &custom_component_ids,
                    );
                    changed_filter_ids.push(id);
                }
                QueryFilter::Added(added) => {
                    let id =
                        register_component_id(world, &added.component_type, &custom_component_ids);
                    added_filter_ids.push(id);
                }
                QueryFilter::Has(_has) => {
                    // Has is handled differently - it's not a filter but a component in the query result
                    // Skip for now
                }
                QueryFilter::AnyOf(anyof) => {
                    // AnyOf is implemented using Bevy's or() builder API
                    // This creates an Or<(With<A>, With<B>, ...)> filter
                    for component_type in &anyof.values {
                        let id =
                            register_component_id(world, component_type, &custom_component_ids);
                        anyof_filter_ids.push(id);
                    }
                }
            }
        }

        // Retain filter IDs for per-entity tick checking (they get moved into QueryBuildSpec)
        let tick_changed_ids = changed_filter_ids.clone();
        let tick_added_ids = added_filter_ids.clone();

        // Build the QueryState once
        let spec = QueryBuildSpec {
            components: component_ids.clone(),
            with_filters: with_filter_ids,
            without_filters: without_filter_ids,
            changed_filters: changed_filter_ids,
            added_filters: added_filter_ids,
            anyof_filters: anyof_filter_ids,
        };
        // Build the correct QueryState variant.
        // Use FilteredEntityRef for all-read-only queries to enable parallel scheduling.
        let query_state = if spec.is_read_only() {
            ErasedQueryState::from_ref(build_query_state_ref(world, &spec))
        } else {
            ErasedQueryState::from_mut(build_query_state(world, &spec))
        };

        // Build component ID cache by mapping TypeId back to PyComponentType
        let mut component_id_cache = HashMap::new();
        let mut component_idx = 0; // Track index in component_ids vec
        for param_type in param.data.iter() {
            if let QueryData::Component { ty, .. } = param_type {
                if let Some(&QueryComponent { id: comp_id, .. }) = component_ids.get(component_idx)
                {
                    // For built-in components, verify by TypeId
                    let type_id = ty.type_id();

                    if let Some(type_id) = type_id {
                        if world
                            .components()
                            .get_info(comp_id)
                            .and_then(|info| info.type_id())
                            .is_some_and(|tid| tid == type_id)
                        {
                            component_id_cache.insert(ty.clone(), comp_id);
                        }
                    } else {
                        // Custom components don't have TypeId - just cache the ID
                        component_id_cache.insert(ty.clone(), comp_id);
                    }
                }
                component_idx += 1; // Increment only for Component params
            }
        }

        // Build parallel array of extraction function pointers for Dynamic components
        let extract_fns: SmallVec<[Option<ExtractFn>; 8]> = param
            .data
            .iter()
            .map(|param_type| {
                if let QueryData::Component {
                    ty: PyComponentType::Dynamic(type_ptr),
                    ..
                } = param_type
                {
                    global_registry::get_bridge_by_py_type(*type_ptr)
                        .map(|bridge| bridge.extract_fn())
                } else {
                    None
                }
            })
            .collect();

        // Create per-parameter access modes
        let param_access_modes: SmallVec<[AccessMode; 8]> = param
            .data
            .iter()
            .map(|param_type| match param_type {
                QueryData::Component { mutable, .. } => {
                    if *mutable {
                        AccessMode::Write
                    } else {
                        AccessMode::Read
                    }
                }
                _ => AccessMode::Read,
            })
            .collect();

        let single_entity_enforced = param.single_entity_enforced;

        Self {
            param,
            query_state,
            component_id_cache,
            custom_component_ids,
            param_access_modes,
            extract_fns,
            changed_filter_ids: tick_changed_ids,
            added_filter_ids: tick_added_ids,
            single_entity_enforced,
        }
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
    /// The query parameter information (Arc-shared clone of the cache's copy).
    /// Kept as its own field so per-entity extraction can iterate `param.data`
    /// while mutating `values_buffer` (disjoint-field borrows).
    param: Arc<PyQueryParam>,

    /// Raw pointer to the static per-parameter state owned by the DynamicSystem.
    /// SAFETY: valid while the ValidityFlag is active; the DynamicSystem outlives the run.
    cached: NonNull<CachedQuery>,

    /// Current lazy iterator state (created on first `__next__` call).
    query_iter: Option<ErasedQueryIter>,

    /// The world cell (lifetime-erased). Copy, valid only during system execution.
    /// SAFETY: fenced by the ValidityFlag.
    world_cell: Option<UnsafeWorldCell<'static>>,

    /// Reusable buffer for return values - avoids allocation on every __next__ call
    /// SmallVec[8] keeps up to 8 items on stack (most queries have 1-4 params)
    values_buffer: SmallVec<[Py<PyAny>; 8]>,

    /// Master validity flag - invalidated when system exits (RAII via ValidityGuard)
    /// All component proxies check this to ensure they're only used during system execution
    validity: ValidityFlag,

    /// True while iteration is in progress (__next__ has been called but not yet exhausted).
    /// Used to detect and reject nested iteration (which would silently corrupt state),
    /// matching Bevy's borrow-checker prevention of nested query.iter() calls.
    iterating: bool,

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

    /// System's last_run tick (from DynamicSystem::get_last_run()).
    last_run: Tick,
    /// Current world change tick for this run (the incremented tick from run_unsafe).
    this_run: Tick,
}

// SAFETY: PyQueryIter is only used during system execution on a single thread.
// The world cell and cached state are only accessed during system execution and never across threads.
// Arc<PyQueryParam> and NonNull<CachedQuery> are fenced by the ValidityFlag.
unsafe impl Send for PyQueryIter {}
unsafe impl Sync for PyQueryIter {}

impl PyQueryIter {
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
        // Erase the cell lifetime for storage; it is only ever used while the
        // system runs, fenced by `validity`.
        // SAFETY: size- and layout-preserving lifetime erasure of a Copy pointer type.
        let world_cell: UnsafeWorldCell<'static> = unsafe { std::mem::transmute(world_cell) };

        Self {
            param: cached.param.clone(),
            cached: NonNull::from(cached),
            query_iter: None,
            world_cell: Some(world_cell),
            values_buffer: SmallVec::new(),
            validity,
            iterating: false,
            layout_cache: RefCell::new(HashMap::new()),
            last_run,
            this_run,
        }
    }

    /// Borrow the static cached query state.
    ///
    /// SAFETY: `cached` points into a CachedQuery owned by the DynamicSystem, which
    /// outlives every run; access is fenced by the same ValidityFlag as `world_cell`.
    #[inline]
    fn cached(&self) -> &CachedQuery {
        unsafe { self.cached.as_ref() }
    }

    /// Raw `*mut World` for the legacy custom-component write-back path.
    ///
    /// SAFETY: the custom-component mutation write-back still needs a `&mut World`
    /// to stamp change ticks (not yet migrated off the raw pointer, slated for a
    /// later stage). The cell is valid while the ValidityFlag is active.
    #[inline]
    fn world_ptr(&self) -> *mut World {
        let cell = self
            .world_cell
            .expect("Query used outside system execution");
        unsafe { cell.world_mut() as *mut World }
    }

    /// Get extraction function pointer for a parameter by index.
    ///
    /// Returns Some(extract_fn) for Dynamic components, None for others.
    /// Uses direct array indexing - O(1) with no HashMap overhead.
    #[inline(always)]
    pub(crate) fn get_extract_fn(&self, param_idx: usize) -> Option<ExtractFn> {
        self.cached().extract_fns.get(param_idx).copied().flatten()
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
        param_idx: usize,
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
                let data_ptr: *mut u8 = {
                    let untyped = entity
                        .get_by_id(component_id)
                        .expect("Custom component should exist on matched entity");
                    unsafe { wrapper_size.get_ref_ptr_as_mut(untyped) }
                };

                let layout = cached_layout.expect("Wrapper storage must have layout");

                // Create lazy wrapper proxy
                let entity_id = entity.id();
                let access_mode = self.cached().param_access_modes[param_idx];
                let validity = self.validity.with_access_mode(access_mode);
                let mutable = access_mode == AccessMode::Write;
                let world_ptr = self.world_ptr();
                let proxy = unsafe {
                    PyLazyWrapperProxy::new(
                        data_ptr,
                        layout,
                        type_ptr,
                        validity,
                        mutable, // true for Mut[T], false for read-only
                        component_id,
                        entity_id,
                        world_ptr,
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

                let py_obj_ptr = unsafe {
                    let py_any_ref = &*(untyped_ptr as *const Py<PyAny>);
                    py_any_ref.as_ptr()
                };

                // Create borrowed reference with validity tracking and entity context
                let access_mode = self.cached().param_access_modes[param_idx];
                let validity = self.validity.with_access_mode(access_mode);
                let world_ptr = self.world_ptr();

                let custom_comp = PyCustomComponent::from_borrowed(
                    py_obj_ptr,
                    validity,
                    component_id,
                    entity_id,
                    world_ptr,
                );

                let py_obj = Py::new(py, (custom_comp, crate::ecs::component::PyComponent))
                    .expect("Failed to create PyCustomComponent");
                Ok(py_obj.into_any())
            }
        }
    }

    /// Extract component data from an entity and populate values_buffer
    ///
    /// This helper consolidates the component extraction logic shared by
    /// __next__(), single(), and get() methods.
    fn extract_components_from_entity(
        &mut self,
        entity: &mut FilteredEntityAccess,
        py: Python,
    ) -> PyResult<()> {
        self.values_buffer.clear();

        for (param_idx, param_type) in self.param.data.iter().enumerate() {
            match param_type {
                QueryData::Entity => {
                    let py_entity = PyEntity(entity.id());
                    let obj = Py::new(py, py_entity).expect("Failed to create PyEntity");
                    self.values_buffer.push(obj.into_any());
                }
                QueryData::Component {
                    ty,
                    mutable: _,
                    optional,
                } => {
                    // Get component ID from cache (handles both built-in and custom components)
                    let component_id = match ty {
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
                    };

                    // For optional components, check if entity has the component
                    if *optional && entity.get_by_id(component_id).is_none() {
                        self.values_buffer.push(py.None());
                        continue;
                    }

                    // Create validity flag with correct access mode
                    let access_mode = self.cached().param_access_modes[param_idx];
                    let validity = self.validity.with_access_mode(access_mode);

                    // Use macro-generated dispatch method (handles all component types)
                    let obj = ty.extract_from_entity(
                        entity,
                        component_id,
                        validity,
                        py,
                        self,
                        param_idx,
                    )?;
                    self.values_buffer.push(obj);
                }
            }
        }

        Ok(())
    }

    /// Returns true if the entity passes all Added/Changed tick filters.
    /// Fast path: returns true immediately when no tick filters exist.
    #[inline]
    fn entity_passes_tick_filters(&self, entity: &FilteredEntityAccess) -> bool {
        let cached = self.cached();
        passes_tick_filters(
            |id| entity.get_change_ticks_by_id(id),
            &cached.changed_filter_ids,
            &cached.added_filter_ids,
            self.last_run,
            self.this_run,
        )
    }

    /// Returns true if there are any tick filters (Added/Changed) on this query.
    #[inline]
    fn has_tick_filters(&self) -> bool {
        let cached = self.cached();
        !cached.changed_filter_ids.is_empty() || !cached.added_filter_ids.is_empty()
    }
}

/// Check whether an entity passes Added/Changed tick filters.
///
/// Generic over the entity type via a closure that provides `get_change_ticks_by_id`.
/// Used by both `FilteredEntityAccess` (for __next__/get/single/iter_many) and
/// raw `FilteredEntityRef`/`FilteredEntityMut` (for __len__/is_empty via iter_manual).
#[inline]
fn passes_tick_filters(
    get_ticks: impl Fn(ComponentId) -> Option<ComponentTicks>,
    changed_ids: &[ComponentId],
    added_ids: &[ComponentId],
    last_run: Tick,
    this_run: Tick,
) -> bool {
    if changed_ids.is_empty() && added_ids.is_empty() {
        return true;
    }

    for &id in changed_ids {
        if let Some(ticks) = get_ticks(id) {
            if !ticks.is_changed(last_run, this_run) {
                return false;
            }
        }
    }

    for &id in added_ids {
        if let Some(ticks) = get_ticks(id) {
            if !ticks.is_added(last_run, this_run) {
                return false;
            }
        }
    }

    true
}

#[pymethods]
impl PyQueryIter {
    /// Makes this object iterable.
    /// Resets the iterator state so that re-iteration works correctly
    /// (matching Bevy's query semantics where each .iter() call is fresh).
    /// Rejects nested iteration (matching Bevy's borrow-checker prevention).
    fn __iter__(slf: Py<Self>, py: Python) -> PyResult<Py<Self>> {
        {
            let mut borrowed = slf.borrow_mut(py);

            // Reject nested iteration — in Bevy Rust this is a borrow error
            if borrowed.iterating {
                return Err(PyRuntimeError::new_err(
                    "Cannot nest iteration on the same Query (Bevy disallows this via borrow rules). \
                     Collect into a list first: items = list(query)",
                ));
            }

            // Reset iterator for sequential re-iteration (Drop handles cleanup)
            borrowed.query_iter.take();
        }
        Ok(slf)
    }

    /// Returns the next query result
    fn __next__(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        // Mark that iteration is in progress (for nested iteration detection)
        self.iterating = true;

        // Create iterator on first call
        if self.query_iter.is_none() {
            let cell = self
                .world_cell
                .expect("Query used outside system execution");
            let (read_only, qs_ptr) = self.cached().query_state.parts();
            // SAFETY: declared access from initialize covers this state; the executor
            // prevents conflicting systems from running concurrently, so the unchecked
            // access is unique. Ticks flow explicitly for per-entity change detection.
            self.query_iter = Some(unsafe {
                erased_create_iter(read_only, qs_ptr, cell, self.last_run, self.this_run)
            });
        }

        // Advance iterator — get raw pointer to avoid borrow conflict with self.
        // Loop to skip entities that don't pass Added/Changed tick filters.
        let iter_ref = self.query_iter.as_mut().unwrap() as *mut ErasedQueryIter;
        // SAFETY: iter_ref is valid; we don't access self.query_iter again until
        // entity_access is dropped (after extract_components_from_entity).
        let entity_access = loop {
            let next = unsafe { (*iter_ref).next() };
            match next {
                Some(access) => {
                    if self.entity_passes_tick_filters(&access) {
                        break Some(access);
                    }
                    // Entity doesn't pass tick filters, skip to next
                }
                None => break None,
            }
        };

        if let Some(mut entity_access) = entity_access {
            // Extract components using the shared helper
            self.extract_components_from_entity(&mut entity_access, py)?;

            // Return single value or tuple based on whether query was Query[T] or Query[tuple[...]]
            if self.param.single {
                Ok(self.values_buffer[0].clone_ref(py))
            } else {
                let tuple = PyTuple::new(py, &self.values_buffer)?;
                Ok(tuple.into_any().unbind())
            }
        } else {
            // Iterator exhausted
            self.iterating = false;
            Err(PyStopIteration::new_err(""))
        }
    }

    /// Returns the number of entities matching the query.
    ///
    /// **Warning: O(n)** - this iterates all matching entities to count them.
    /// Python users calling `len(query)` may expect O(1) but Bevy's
    /// `QueryState` does not cache entity counts.
    fn __len__(&self) -> usize {
        let Some(cell) = self.world_cell else {
            return 0;
        };

        let cached = self.cached();
        if !self.has_tick_filters() {
            return cached.query_state.count(cell, self.last_run, self.this_run);
        }

        // Count with tick filtering: iterate the cell-based unchecked iterator and
        // apply the per-entity Changed/Added checks.
        let (read_only, qs_ptr) = cached.query_state.parts();
        // SAFETY: declared access from initialize covers this state; the executor
        // prevents conflicting systems from running concurrently, so the unchecked
        // access is unique.
        let mut iter =
            unsafe { erased_create_iter(read_only, qs_ptr, cell, self.last_run, self.this_run) };
        let mut n = 0usize;
        while let Some(access) = iter.next() {
            if self.entity_passes_tick_filters(&access) {
                n += 1;
            }
        }
        n
    }

    /// Get exactly one entity from the query.
    /// Returns an error if there are 0 or 2+ entities matching the query.
    fn single(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        let cell = self
            .world_cell
            .expect("Query used outside system execution");
        let last_run = self.last_run;
        let this_run = self.this_run;
        let (read_only, qs_ptr) = self.cached().query_state.parts();

        // Validate exactly one entity exists and get it for extraction.
        // Loop through entities to find those passing tick filters.
        // SAFETY: declared access from initialize covers this state; the executor
        // prevents conflicting systems from running concurrently, so the unchecked
        // access is unique. Raw pointer casts avoid holding borrows across extraction.
        let mut entity_access: FilteredEntityAccess = unsafe {
            if read_only {
                let qs = &mut *(qs_ptr as *mut QueryState<FilteredEntityRef>);
                let mut iter = qs
                    .query_unchecked_with_ticks(cell, last_run, this_run)
                    .iter_inner();
                // Find first entity passing tick filters
                let first = loop {
                    match iter.next() {
                        Some(e) => {
                            let access = FilteredEntityAccess::Ref(e);
                            if self.entity_passes_tick_filters(&access) {
                                break Some(access);
                            }
                        }
                        None => break None,
                    }
                };
                // Check for second passing entity
                let has_second = loop {
                    match iter.next() {
                        Some(e) => {
                            if self.entity_passes_tick_filters(&FilteredEntityAccess::Ref(e)) {
                                break true;
                            }
                        }
                        None => break false,
                    }
                };
                match (first, has_second) {
                    (None, _) => {
                        return Err(PyRuntimeError::new_err(
                            "Query returned no entities. Expected exactly one.",
                        ));
                    }
                    (Some(_), true) => {
                        return Err(PyRuntimeError::new_err(
                            "Query returned multiple entities. Expected exactly one.",
                        ));
                    }
                    (Some(e), false) => e,
                }
            } else {
                let qs = &mut *(qs_ptr as *mut QueryState<FilteredEntityMut>);
                let mut iter = qs
                    .query_unchecked_with_ticks(cell, last_run, this_run)
                    .iter_inner();
                // Find first entity passing tick filters
                let first = loop {
                    match iter.next() {
                        Some(e) => {
                            let access = FilteredEntityAccess::Mut(e);
                            if self.entity_passes_tick_filters(&access) {
                                break Some(access);
                            }
                        }
                        None => break None,
                    }
                };
                // Check for second passing entity
                let has_second = loop {
                    match iter.next() {
                        Some(e) => {
                            if self.entity_passes_tick_filters(&FilteredEntityAccess::Mut(e)) {
                                break true;
                            }
                        }
                        None => break false,
                    }
                };
                match (first, has_second) {
                    (None, _) => {
                        return Err(PyRuntimeError::new_err(
                            "Query returned no entities. Expected exactly one.",
                        ));
                    }
                    (Some(_), true) => {
                        return Err(PyRuntimeError::new_err(
                            "Query returned multiple entities. Expected exactly one.",
                        ));
                    }
                    (Some(e), false) => e,
                }
            }
        };

        self.extract_components_from_entity(&mut entity_access, py)?;

        if self.param.single {
            Ok(self.values_buffer[0].clone_ref(py))
        } else {
            let tuple = PyTuple::new(py, &self.values_buffer)?;
            Ok(tuple.into_any().unbind())
        }
    }

    /// Check if the query has no matching entities.
    /// Returns true if there are no entities matching the query filters.
    fn is_empty(&self) -> PyResult<bool> {
        let cell = self
            .world_cell
            .expect("Query used outside system execution");
        let cached = self.cached();

        if !self.has_tick_filters() {
            return Ok(cached
                .query_state
                .is_empty_check(cell, self.last_run, self.this_run));
        }

        // Check with tick filtering: iterate the cell-based unchecked iterator and
        // stop at the first entity that passes the per-entity Changed/Added checks.
        let (read_only, qs_ptr) = cached.query_state.parts();
        // SAFETY: declared access from initialize covers this state; the executor
        // prevents conflicting systems from running concurrently, so the unchecked
        // access is unique.
        let mut iter =
            unsafe { erased_create_iter(read_only, qs_ptr, cell, self.last_run, self.this_run) };
        while let Some(access) = iter.next() {
            if self.entity_passes_tick_filters(&access) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Get components for a specific entity by ID.
    /// Returns None if the entity doesn't match the query filters.
    /// Returns an error if the entity doesn't have the queried components.
    fn get(&mut self, entity: PyEntity, py: Python) -> PyResult<Option<Py<PyAny>>> {
        let cell = self
            .world_cell
            .expect("Query used outside system execution");
        let (read_only, qs_ptr) = self.cached().query_state.parts();
        // SAFETY: declared access from initialize covers this state; the executor
        // prevents conflicting systems from running concurrently, so the unchecked
        // access is unique.
        let result = unsafe {
            erased_get_entity(
                read_only,
                qs_ptr,
                cell,
                self.last_run,
                self.this_run,
                entity.0,
            )
        };

        match result {
            Some(mut entity_access) => {
                // Check tick filters before extracting
                if !self.entity_passes_tick_filters(&entity_access) {
                    return Ok(None);
                }

                self.extract_components_from_entity(&mut entity_access, py)?;

                if self.param.single {
                    Ok(Some(self.values_buffer[0].clone_ref(py)))
                } else {
                    let tuple = PyTuple::new(py, &self.values_buffer)?;
                    Ok(Some(tuple.into_any().unbind()))
                }
            }
            None => Ok(None),
        }
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
        let mut results = Vec::new();
        let cell = self
            .world_cell
            .expect("Query used outside system execution");
        let (read_only, qs_ptr) = self.cached().query_state.parts();
        let (last_run, this_run) = (self.last_run, self.this_run);

        for entity_obj in entities.try_iter()? {
            let entity_id: PyEntity = entity_obj?.extract()?;

            // SAFETY: declared access from initialize covers this state; the executor
            // prevents conflicting systems from running concurrently, so the unchecked
            // access is unique.
            if let Some(mut access) = unsafe {
                erased_get_entity(read_only, qs_ptr, cell, last_run, this_run, entity_id.0)
            } {
                // Check tick filters before extracting
                if !self.entity_passes_tick_filters(&access) {
                    continue;
                }
                self.extract_components_from_entity(&mut access, py)?;
                let result = if self.param.single {
                    self.values_buffer[0].clone_ref(py)
                } else {
                    let tuple =
                        PyTuple::new(py, &self.values_buffer).expect("Failed to create tuple");
                    tuple.into_any().unbind()
                };
                results.push(result);
            }
        }

        Ok(results)
    }
}
