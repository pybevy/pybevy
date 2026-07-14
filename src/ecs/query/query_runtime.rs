use std::{cell::RefCell, collections::HashMap, ptr::NonNull, sync::Arc};

#[cfg(debug_assertions)]
use bevy::ecs::query::FilteredAccess;
use bevy::{
    ecs::{
        change_detection::Tick, component::ComponentId, world::unsafe_world_cell::UnsafeWorldCell,
    },
    prelude::*,
};
use pybevy_core::{ExtractFn, FilteredEntityAccess, registry::global_registry};
use pybevy_ecs::shared::{
    cached_query::CachedQueryCore,
    query_builder_ext::{QueryBuildSpec, QueryComponent},
    query_runtime::{QueryExecutionError, QueryRuntimeCore, QueryRuntimeError, RowMaterializer},
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
    component_layout::{ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt},
    component_type::{PyComponentType, register_component_id},
    filter::QueryFilter,
    helpers::validity_guard::{AccessMode, ValidityFlag},
    lazy_wrapper_proxy::{ProxyKind, PyLazyWrapperProxy},
    query::query_param::{PyQueryParam, QueryData},
};

fn query_runtime_error_to_py(error: QueryRuntimeError) -> PyErr {
    match error {
        QueryRuntimeError::Storage(error) => error.into(),
        error => PyRuntimeError::new_err(error.to_string()),
    }
}

fn query_execution_error_to_py(error: QueryExecutionError<PyErr>) -> PyErr {
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

    /// Per-parameter access modes (Read or Write), indexed by parameter position.
    param_access_modes: SmallVec<[AccessMode; 8]>,

    /// Extraction function pointers for Dynamic components, indexed by parameter position.
    extract_fns: SmallVec<[Option<ExtractFn>; 8]>,

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

        // Build the QueryState once. `build_auto` picks FilteredEntityRef for
        // all-read-only queries and retains the tick-filter ids for the
        // per-entity Changed/Added check.
        let spec = QueryBuildSpec {
            components: component_ids.clone(),
            with_filters: with_filter_ids,
            without_filters: without_filter_ids,
            changed_filters: changed_filter_ids,
            added_filters: added_filter_ids,
            anyof_filters: anyof_filter_ids,
        };
        let core = CachedQueryCore::build_auto(world, &spec);

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
            core,
            component_id_cache,
            custom_component_ids,
            param_access_modes,
            extract_fns,
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

    /// Raw pointer to the static per-parameter state owned by the DynamicSystem.
    /// SAFETY: valid while the ValidityFlag is active; the DynamicSystem outlives the run.
    cached: NonNull<CachedQuery>,

    /// Interpreter-neutral iteration, lookup, tick-filter, and validity state.
    runtime: QueryRuntimeCore,

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
// SAFETY: every Python-visible operation takes PyO3's exclusive borrow (`&mut
// self`, or `borrow_mut` for `__iter__`). That runtime borrow prevents concurrent
// access on free-threaded Python even though PyO3 requires the pyclass to be Sync.
// Row materialization is only entered while one of those exclusive operations
// holds the object borrow.
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
        // SAFETY: this constructor carries the same stable-cache, matching-world,
        // and validity-window contract as QueryRuntimeCore::new.
        let runtime = unsafe {
            QueryRuntimeCore::new(Some(&cached.core), world_cell, validity, last_run, this_run)
        };

        Self {
            param: cached.param.clone(),
            cached: NonNull::from(cached),
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
        unsafe { self.cached.as_ref() }
    }

    /// Raw `*mut World` for the legacy custom-component write-back path.
    ///
    /// SAFETY: the custom-component mutation write-back still needs a `&mut World`
    /// to stamp change ticks (not yet migrated off the raw pointer, slated for a
    /// later stage). The cell is valid while the ValidityFlag is active.
    #[inline]
    fn world_ptr(&self) -> PyResult<*mut World> {
        // SAFETY: this is the documented custom-component compatibility path;
        // validity and scheduler access constrain its residual whole-world pointer.
        unsafe { self.runtime.world_ptr() }.map_err(query_runtime_error_to_py)
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
                let validity = self.runtime.validity().with_access_mode(access_mode);
                let mutable = access_mode == AccessMode::Write;
                let world_ptr = self.world_ptr()?;
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

                let py_obj_ptr = unsafe {
                    let py_any_ref = &*(untyped_ptr as *const Py<PyAny>);
                    py_any_ref.as_ptr()
                };

                // Create borrowed reference with validity tracking and entity context
                let access_mode = self.cached().param_access_modes[param_idx];
                let validity = self.runtime.validity().with_access_mode(access_mode);
                let world_ptr = self.world_ptr()?;

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
        &self,
        entity: &mut FilteredEntityAccess,
        py: Python,
    ) -> PyResult<()> {
        let mut values_buffer = self.values_buffer.borrow_mut();
        values_buffer.clear();

        for (param_idx, param_type) in self.param.data.iter().enumerate() {
            match param_type {
                QueryData::Entity => {
                    let py_entity = PyEntity(entity.id());
                    let obj = Py::new(py, py_entity).expect("Failed to create PyEntity");
                    values_buffer.push(obj.into_any());
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
                        values_buffer.push(py.None());
                        continue;
                    }

                    // Create validity flag with correct access mode
                    let access_mode = self.cached().param_access_modes[param_idx];
                    let validity = self.runtime.validity().with_access_mode(access_mode);

                    // Use macro-generated dispatch method (handles all component types)
                    let obj = ty.extract_from_entity(
                        entity,
                        component_id,
                        validity,
                        py,
                        self,
                        param_idx,
                    )?;
                    values_buffer.push(obj);
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
}

/// Private row adapter. Keeping the public neutral trait off `PyQueryIter`
/// ensures row materialization cannot bypass PyO3's exclusive object borrow.
struct PyQueryRowMaterializer<'a> {
    query: &'a PyQueryIter,
}

impl<'py> RowMaterializer<Python<'py>> for PyQueryRowMaterializer<'_> {
    type Output = Py<PyAny>;
    type Error = PyErr;

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
    /// Makes this object iterable.
    /// Resets the iterator state so that re-iteration works correctly
    /// (matching Bevy's query semantics where each .iter() call is fresh).
    /// Rejects nested iteration (matching Bevy's borrow-checker prevention).
    fn __iter__(slf: Py<Self>, py: Python) -> PyResult<Py<Self>> {
        {
            let borrowed = slf.borrow_mut(py);
            borrowed
                .runtime
                .begin_iteration()
                .map_err(query_runtime_error_to_py)?;
        }
        Ok(slf)
    }

    /// Returns the next query result
    fn __next__(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        let materializer = PyQueryRowMaterializer { query: self };
        if let Some(row) = self
            .runtime
            .next_with(&materializer, py)
            .map_err(query_execution_error_to_py)?
        {
            Ok(row)
        } else {
            Err(PyStopIteration::new_err(""))
        }
    }

    /// Returns the number of entities matching the query.
    ///
    /// **Warning: O(n)** - this iterates all matching entities to count them.
    /// Python users calling `len(query)` may expect O(1) but Bevy's
    /// `QueryState` does not cache entity counts.
    fn __len__(&mut self) -> PyResult<usize> {
        self.runtime.count().map_err(query_runtime_error_to_py)
    }

    /// Get exactly one entity from the query.
    /// Returns an error if there are 0 or 2+ entities matching the query.
    fn single(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        let materializer = PyQueryRowMaterializer { query: self };
        self.runtime
            .single_with(&materializer, py)
            .map_err(query_execution_error_to_py)
    }

    /// Check if the query has no matching entities.
    /// Returns true if there are no entities matching the query filters.
    fn is_empty(&mut self) -> PyResult<bool> {
        self.runtime.is_empty().map_err(query_runtime_error_to_py)
    }

    /// Get components for a specific entity by ID.
    /// Returns None if the entity doesn't match the query filters.
    /// Returns an error if the entity doesn't have the queried components.
    fn get(&mut self, entity: PyEntity, py: Python) -> PyResult<Option<Py<PyAny>>> {
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
        let materializer = PyQueryRowMaterializer { query: self };

        for entity_obj in entities.try_iter()? {
            let entity_id: PyEntity = entity_obj?.extract()?;
            if let Some(row) = self
                .runtime
                .get_with(entity_id.0, &materializer, py)
                .map_err(query_execution_error_to_py)?
            {
                results.push(row);
            }
        }

        Ok(results)
    }
}
