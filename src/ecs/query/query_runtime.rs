use std::{cell::RefCell, collections::HashMap, ptr::NonNull, sync::Arc};

use bevy::{
    ecs::{
        component::ComponentId,
        query::{QueryBuilder, QueryIter, QueryState},
        world::FilteredEntityMut,
    },
    prelude::*,
};
use pybevy_core::{ExtractFn, registry::global_registry};
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

/// Runtime query iterator that can be passed to Python systems.
/// Uses Bevy's QueryState for efficient cached iteration.
///
/// SAFETY: This struct uses unsafe transmute to erase the lifetime from QueryState.
/// It must only be used within the scope of a system execution and must not escape
/// the Python GIL callback. Python code must not store references to this object
/// or any iterators derived from it beyond the system function scope.
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
    /// The query parameter information (shared via Arc to avoid clones)
    param: Arc<PyQueryParam>,

    /// The Bevy QueryState with lifetime erased via transmute
    /// SAFETY: This is created from a QueryState<FilteredEntityMut<'w, 's>> in new()
    /// The actual type is QueryState<FilteredEntityMut<'static, 'static>> due to transmute
    /// but the real lifetimes are tied to the system execution and guaranteed by caller
    query_state_ptr: *mut (),

    /// Current iterator state (also lifetime-erased)
    /// Stores the actual Bevy QueryIter - we call .next() on it incrementally
    /// Type is QueryIter<'w, 's, FilteredEntityMut, ()> with lifetime erased
    iterator_ptr: Option<*mut ()>,

    /// Raw pointer to the World (only valid during system execution)
    /// SAFETY: This pointer is only valid within the scope of the system execution
    world_ptr: Option<NonNull<World>>,

    /// Maps PyComponentType to their registered ComponentIds (cached for fast access)
    /// Stores component IDs for all queried components for efficient lookup
    component_id_cache: HashMap<PyComponentType, ComponentId>,

    /// Maps custom component type pointers to their registered ComponentIds (shared via Arc)
    custom_component_ids: Arc<HashMap<*const PyTypeObject, ComponentId>>,

    /// Reusable buffer for return values - avoids allocation on every __next__ call
    /// SmallVec[8] keeps up to 8 items on stack (most queries have 1-4 params)
    values_buffer: SmallVec<[Py<PyAny>; 8]>,

    /// Master validity flag - invalidated when system exits (RAII via ValidityGuard)
    /// All component proxies check this to ensure they're only used during system execution
    validity: ValidityFlag,

    /// Per-parameter access modes (Read or Write)
    /// Indexed by parameter position, determines if a component can be read-only or mutated
    param_access_modes: SmallVec<[AccessMode; 8]>,

    /// Extraction function pointers for Dynamic components, indexed by parameter position.
    /// None for non-Dynamic parameters, Some(fn) for Dynamic components.
    /// This eliminates HashMap lookup overhead during per-entity iteration.
    extract_fns: SmallVec<[Option<ExtractFn>; 8]>,

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
}

impl Drop for PyQueryIter {
    fn drop(&mut self) {
        // Clean up the QueryState
        if !self.query_state_ptr.is_null() {
            unsafe {
                // SAFETY: We created this from a valid QueryState in new()
                let _ = Box::from_raw(self.query_state_ptr as *mut QueryState<FilteredEntityMut>);
            }
        }
        // Clean up the iterator if it exists
        if let Some(iter_ptr) = self.iterator_ptr
            && !iter_ptr.is_null() {
                unsafe {
                    // SAFETY: We created this from a valid QueryIter
                    let _ = Box::from_raw(iter_ptr as *mut QueryIter<FilteredEntityMut, ()>);
                }
            }
    }
}

// SAFETY: PyQueryIter is only used during system execution on a single thread.
// The world pointer and query state are only accessed during system execution and never across threads.
// Arc<PyQueryParam> and Arc<HashMap> are already Send/Sync.
unsafe impl Send for PyQueryIter {}
unsafe impl Sync for PyQueryIter {}

impl PyQueryIter {
    /// Creates a new runtime query from a Bevy world
    ///
    /// SAFETY: The world pointer must remain valid for the lifetime of this object
    pub unsafe fn new(
        param: Arc<PyQueryParam>,
        world: &mut World,
        custom_component_ids: Arc<HashMap<*const PyTypeObject, ComponentId>>,
        validity: ValidityFlag,
    ) -> Self {
        // First, collect and register all component IDs (tracking optional status)
        let mut component_ids = Vec::new();
        for param_type in &param.data {
            if let QueryData::Component {
                    ty: comp_type,
                    optional,
                    ..
                } = param_type {
                let id = register_component_id(world, comp_type, &custom_component_ids);
                component_ids.push((id, *optional));
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

        // Build the QueryState once - this will be cached and reused for efficient iteration
        let mut builder = QueryBuilder::<FilteredEntityMut>::new(world);

        for &(id, optional) in component_ids.iter() {
            if optional {
                builder.optional(|b| {
                    b.mut_id(id);
                });
            } else {
                builder.mut_id(id);
            }
        }

        for &id in with_filter_ids.iter() {
            builder.with_id(id);
        }

        for &id in without_filter_ids.iter() {
            builder.without_id(id);
        }

        for &id in changed_filter_ids.iter() {
            builder.ref_id(id);
        }

        for &id in added_filter_ids.iter() {
            builder.ref_id(id);
        }

        // Apply AnyOf filter using or() builder API
        if !anyof_filter_ids.is_empty() {
            builder.or(|b| {
                for &id in anyof_filter_ids.iter() {
                    b.with_id(id);
                }
            });
        }

        let query_state = builder.build();

        // SAFETY: Transmute to erase lifetime - the caller guarantees this is only used
        // within the system execution scope where the World reference is valid
        let query_state_boxed = Box::new(query_state);
        let query_state_ptr = Box::into_raw(query_state_boxed) as *mut ();

        // Build component ID cache by mapping TypeId back to PyComponentType
        let mut component_id_cache = HashMap::new();
        let mut component_idx = 0; // Track index in component_ids vec
        for param_type in param.data.iter() {
            if let QueryData::Component { ty, .. } = param_type {
                // Get the corresponding ComponentId from the component_ids vec
                if let Some(&(comp_id, _optional)) = component_ids.get(component_idx) {
                    // For built-in components, verify by TypeId
                    let type_id = ty.type_id();

                    if let Some(type_id) = type_id {
                        // Verify this is the right component by checking TypeId
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

        // Use the provided validity flag - shared with other system parameters
        // This will be automatically invalidated when the system completes

        // Build parallel array of extraction function pointers for Dynamic components
        // This eliminates HashMap lookup overhead during per-entity iteration
        let extract_fns: SmallVec<[Option<ExtractFn>; 8]> = param
            .data
            .iter()
            .map(|param_type| {
                if let QueryData::Component { ty: PyComponentType::Dynamic(type_ptr), .. } = param_type {
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
            .map(|param_type| {
                match param_type {
                    QueryData::Component { mutable, .. } => {
                        if *mutable {
                            AccessMode::Write
                        } else {
                            AccessMode::Read
                        }
                    }
                    _ => AccessMode::Read, // Default to read for non-component params
                }
            })
            .collect();

        Self {
            param,
            query_state_ptr,
            iterator_ptr: None,
            world_ptr: Some(NonNull::from(world)),
            component_id_cache,
            custom_component_ids,
            values_buffer: SmallVec::new(),
            validity,
            param_access_modes,
            extract_fns,
            iterating: false,
            layout_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Get extraction function pointer for a parameter by index.
    ///
    /// Returns Some(extract_fn) for Dynamic components, None for others.
    /// Uses direct array indexing - O(1) with no HashMap overhead.
    #[inline(always)]
    pub(crate) fn get_extract_fn(&self, param_idx: usize) -> Option<ExtractFn> {
        self.extract_fns.get(param_idx).copied().flatten()
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
        entity_mut: &mut FilteredEntityMut,
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
                    let untyped = entity_mut
                        .get_by_id(component_id)
                        .expect("Custom component should exist on matched entity");
                    unsafe { wrapper_size.get_ref_ptr_as_mut(untyped) }
                };

                let layout = cached_layout.expect("Wrapper storage must have layout");

                // Create lazy wrapper proxy
                let entity = entity_mut.id();
                let access_mode = self.param_access_modes[param_idx];
                let validity = self.validity.with_access_mode(access_mode);
                let mutable = access_mode == AccessMode::Write;
                let world_ptr = self
                    .world_ptr
                    .expect("Query used outside system execution")
                    .as_ptr();
                let proxy = unsafe {
                    PyLazyWrapperProxy::new(
                        data_ptr,
                        layout,
                        type_ptr,
                        validity,
                        mutable, // true for Mut[T], false for read-only
                        component_id,
                        entity,
                        world_ptr,
                    )
                };

                let py_obj = Py::new(py, proxy).expect("Failed to create lazy wrapper proxy");
                Ok(py_obj.into_any())
            }
            ComponentStorageType::PyObject => {
                // PyObject storage - return borrowed reference to ECS-stored Python object
                use crate::ecs::custom_component::PyCustomComponent;

                let entity = entity_mut.id();

                // Get pointer to the PyAny in ECS storage
                // SAFETY: We know this is a Py<PyAny> because that's how we registered it
                // NOTE: We use get_by_id() for both mutable and immutable access.
                // Change detection is handled by __setattr__ hook + stored entity context.
                let untyped_ptr = entity_mut
                    .get_by_id(component_id)
                    .expect("Custom component should exist on matched entity")
                    .as_ptr();

                let py_obj_ptr = unsafe {
                    let py_any_ref = &*(untyped_ptr as *const Py<PyAny>);
                    py_any_ref.as_ptr()
                };

                // Create borrowed reference with validity tracking and entity context
                let access_mode = self.param_access_modes[param_idx];
                let validity = self.validity.with_access_mode(access_mode);
                let world_ptr = self
                    .world_ptr
                    .expect("Query used outside system execution")
                    .as_ptr();

                let custom_comp = PyCustomComponent::from_borrowed(
                    py_obj_ptr,
                    validity,
                    component_id,
                    entity,
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
        entity_mut: &mut FilteredEntityMut,
        py: Python,
    ) -> PyResult<()> {
        self.values_buffer.clear();

        for (param_idx, param_type) in self.param.data.iter().enumerate() {
            match param_type {
                QueryData::Entity => {
                    let py_entity = PyEntity(entity_mut.id());
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
                            .custom_component_ids
                            .get(type_ptr)
                            .expect("Custom component ID should be registered"),
                        _ => *self
                            .component_id_cache
                            .get(ty)
                            .expect("Component ID should be cached"),
                    };

                    // For optional components, check if entity has the component
                    if *optional && entity_mut.get_by_id(component_id).is_none() {
                        self.values_buffer.push(py.None());
                        continue;
                    }

                    // Create validity flag with correct access mode
                    let access_mode = self.param_access_modes[param_idx];
                    let validity = self.validity.with_access_mode(access_mode);

                    // Use macro-generated dispatch method (handles all component types)
                    let obj = ty.extract_from_entity(
                        entity_mut,
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

            // Reset iterator for sequential re-iteration
            if let Some(iter_ptr) = borrowed.iterator_ptr.take()
                && !iter_ptr.is_null() {
                    unsafe {
                        let _ = Box::from_raw(iter_ptr as *mut QueryIter<FilteredEntityMut, ()>);
                    }
                }
        }
        Ok(slf)
    }

    /// Returns the next query result
    fn __next__(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        // Clear previous entity's context
        crate::ecs::change_tracking::clear_entity_context();

        // Mark that iteration is in progress (for nested iteration detection)
        self.iterating = true;

        // Create iterator on first call
        if self.iterator_ptr.is_none() {
            // SAFETY: world_ptr is guaranteed to be valid during system execution
            let world = unsafe {
                self.world_ptr
                    .expect("Query used outside system execution")
                    .as_mut()
            };

            // SAFETY: We transmuted this pointer from a valid QueryState in new()
            // and the lifetime is tied to the system execution
            let query_state =
                unsafe { &mut *(self.query_state_ptr as *mut QueryState<FilteredEntityMut>) };

            // Create the iterator - NO pre-collection, truly lazy
            let iter = query_state.iter_mut(world);
            let boxed = Box::new(iter);
            self.iterator_ptr = Some(Box::into_raw(boxed) as *mut ());
        }

        // SAFETY: We created this as QueryIter in the block above
        let iter =
            unsafe { &mut *(self.iterator_ptr.unwrap() as *mut QueryIter<FilteredEntityMut, ()>) };

        // Call .next() on the Bevy iterator - truly incremental
        if let Some(mut entity_mut) = iter.next() {
            let entity = entity_mut.id();

            // Set entity context for lazy change tracking
            // SAFETY: world_ptr is valid during query iteration
            let world_ptr = self
                .world_ptr
                .expect("Query used outside system execution")
                .as_ptr();
            crate::ecs::change_tracking::set_entity_context(entity, world_ptr);

            // Extract components using the shared helper
            self.extract_components_from_entity(&mut entity_mut, py)?;

            // Return single value or tuple based on whether query was Query[T] or Query[tuple[...]]
            if self.param.single {
                Ok(self.values_buffer[0].clone_ref(py))
            } else {
                let tuple = PyTuple::new(py, &self.values_buffer)?;
                Ok(tuple.into_any().unbind())
            }
        } else {
            // Iterator exhausted - clear final entity context and iteration flag
            self.iterating = false;
            crate::ecs::change_tracking::clear_entity_context();
            Err(PyStopIteration::new_err(""))
        }
    }

    /// Returns the number of entities matching the query
    /// Note: Since we use a lazy iterator, this requires iterating through
    /// all remaining entities to count them, which consumes the iterator.
    /// It's better to avoid calling len() if possible.
    fn __len__(&self) -> usize {
        let world = match self.world_ptr {
            Some(ptr) => unsafe { ptr.as_ref() },
            None => return 0,
        };
        if self.query_state_ptr.is_null() {
            return 0;
        }
        let query_state =
            unsafe { &*(self.query_state_ptr as *const QueryState<FilteredEntityMut>) };
        // SAFETY: We only need a read-only count; iter_manual requires &World
        query_state.iter_manual(world).count()
    }

    /// Get exactly one entity from the query.
    /// Returns an error if there are 0 or 2+ entities matching the query.
    fn single(&mut self, py: Python) -> PyResult<Py<PyAny>> {
        // SAFETY: world_ptr is guaranteed to be valid during system execution
        let world = unsafe {
            self.world_ptr
                .expect("Query used outside system execution")
                .as_mut()
        };

        // SAFETY: We transmuted this pointer from a valid QueryState in new()
        let query_state =
            unsafe { &mut *(self.query_state_ptr as *mut QueryState<FilteredEntityMut>) };

        // Collect all matching entities
        let mut iter = query_state.iter_mut(world);

        let first = iter.next();
        let second = iter.next();

        match (first, second) {
            (None, _) => Err(PyRuntimeError::new_err(
                "Query returned no entities. Expected exactly one.",
            )),
            (Some(_), Some(_)) => Err(PyRuntimeError::new_err(
                "Query returned multiple entities. Expected exactly one.",
            )),
            (Some(mut entity_mut), None) => {
                // Exactly one entity - extract components
                let entity = entity_mut.id();

                // Set entity context for lazy change tracking
                // SAFETY: world_ptr is valid during query iteration
                let world_ptr = self
                    .world_ptr
                    .expect("Query used outside system execution")
                    .as_ptr();
                crate::ecs::change_tracking::set_entity_context(entity, world_ptr);

                self.values_buffer.clear();

                self.extract_components_from_entity(&mut entity_mut, py)?;

                // Return single value or tuple based on whether query was Query[T] or Query[tuple[...]]
                if self.param.single {
                    Ok(self.values_buffer[0].clone_ref(py))
                } else {
                    let tuple = PyTuple::new(py, &self.values_buffer)?;
                    Ok(tuple.into_any().unbind())
                }
            }
        }
    }

    /// Check if the query has no matching entities.
    /// Returns true if there are no entities matching the query filters.
    fn is_empty(&self) -> PyResult<bool> {
        // SAFETY: world_ptr is guaranteed to be valid during system execution
        let world = unsafe {
            self.world_ptr
                .expect("Query used outside system execution")
                .as_mut()
        };

        // Get the ticks before borrowing world for the query
        let last_tick = world.last_change_tick();
        let current_tick = world.change_tick();

        // SAFETY: We transmuted this pointer from a valid QueryState in new()
        let query_state =
            unsafe { &*(self.query_state_ptr as *const QueryState<FilteredEntityMut>) };

        Ok(query_state.is_empty(world, last_tick, current_tick))
    }

    /// Get components for a specific entity by ID.
    /// Returns None if the entity doesn't match the query filters.
    /// Returns an error if the entity doesn't have the queried components.
    fn get(&mut self, entity: PyEntity, py: Python) -> PyResult<Option<Py<PyAny>>> {
        // SAFETY: world_ptr is guaranteed to be valid during system execution
        let world = unsafe {
            self.world_ptr
                .expect("Query used outside system execution")
                .as_mut()
        };

        // SAFETY: We transmuted this pointer from a valid QueryState in new()
        let query_state =
            unsafe { &mut *(self.query_state_ptr as *mut QueryState<FilteredEntityMut>) };

        // Try to get the specific entity
        match query_state.get_mut(world, entity.0) {
            Ok(mut entity_mut) => {
                self.extract_components_from_entity(&mut entity_mut, py)?;

                // Return single value or tuple based on whether query was Query[T] or Query[tuple[...]]
                if self.param.single {
                    Ok(Some(self.values_buffer[0].clone_ref(py)))
                } else {
                    let tuple = PyTuple::new(py, &self.values_buffer)?;
                    Ok(Some(tuple.into_any().unbind()))
                }
            }
            Err(_) => {
                // Entity doesn't match query filters
                Ok(None)
            }
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
        // SAFETY: world_ptr is guaranteed to be valid during system execution
        let world = unsafe {
            self.world_ptr
                .expect("Query used outside system execution")
                .as_mut()
        };

        // SAFETY: We transmuted this pointer from a valid QueryState in new()
        let query_state =
            unsafe { &mut *(self.query_state_ptr as *mut QueryState<FilteredEntityMut>) };

        let mut results = Vec::new();

        // Iterate over the provided entities
        for entity_obj in entities.try_iter()? {
            let entity_obj = entity_obj?;
            let entity_id: PyEntity = entity_obj.extract()?;

            // Try to get this specific entity
            match query_state.get_mut(world, entity_id.0) {
                Ok(mut entity_mut) => {
                    self.extract_components_from_entity(&mut entity_mut, py)?;

                    // Return single value or tuple based on whether query was Query[T] or Query[tuple[...]]
                    let result = if self.param.single {
                        self.values_buffer[0].clone_ref(py)
                    } else {
                        let tuple =
                            PyTuple::new(py, &self.values_buffer).expect("Failed to create tuple");
                        tuple.into_any().unbind()
                    };

                    results.push(result);
                }
                Err(_) => {
                    // Entity doesn't match query filters - skip it
                    continue;
                }
            }
        }

        Ok(results)
    }
}
