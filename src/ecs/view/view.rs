//! View API for high-performance batch operations on components.
//!
//! The View API provides a "fast path" for bulk mathematical operations on components,
//! compiling Python expressions to native bytecode executed in parallel loops.
//!
//! # Example Usage
//!
//! ```python
//! def physics_system(view: View[tuple[Mut[Position], Velocity]], time: Time):
//!     pos = view.column_mut(Position)  # Requires Mut[Position] in View type
//!     vel = view.column(Velocity)       # Read-only access
//!
//!     # This compiles to a single parallel loop
//!     pos.x = pos.x + vel.x * time.delta_secs()
//! ```
//!
//! **Important**: Use `Mut[T]` in the View type parameter to declare mutable access,
//! just like Query. This ensures type safety and correct ECS access tracking.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ptr::NonNull,
    sync::Arc,
};

use smallvec::SmallVec;

/// Maximum number of field pointers to stack-allocate before falling back to heap.
/// Most expressions use 1-4 fields, so 8 provides headroom without heap allocation.
type FieldPtrVec = SmallVec<[*mut u8; 8]>;

/// Send+Sync wrapper for raw pointer batch execution.
/// SAFETY: The pointers are valid for the duration of batch execution
/// and the data they point to is accessed in parallel using proper rayon semantics.
#[derive(Clone, Copy)]
struct SendPtr(*mut u8);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

use bevy::{
    ecs::{
        change_detection::Tick,
        component::ComponentId,
        query::QueryBuilder,
        storage::TableId,
        world::{FilteredEntityMut, World},
    },
    prelude::*,
};
use pybevy_bytecodevm::{
    bytecode::{CompiledBytecode, Compiler, FieldId, VM},
    expr::RustExpr,
};
use pybevy_core::PyEntity;
use pyo3::{
    exceptions::{PyAttributeError, PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyAny, PyType},
};

use crate::ecs::{
    component_layout::ComponentStorageType,
    component_type::{PyComponentType, register_component_id_simple},
    helpers::validity_guard::ValidityFlag,
    view::{construct_view_class_item, view_column::PyViewColumn},
};

/// A unique key for caching compiled bytecode
/// Uses Python object identity for fast cache lookups
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExprKey {
    /// The destination field being assigned to
    dest_field: (ComponentId, usize), // (component_id, offset)
    /// Python object identity (memory address) for the expression
    /// This is valid because the same expression object always produces the same bytecode
    expr_identity: usize,
}

/// Secondary cache key using expression hash for when identity doesn't match
/// This handles cases where the same expression is recreated (different object, same semantics)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExprHashKey {
    /// The destination field being assigned to
    dest_field: (ComponentId, usize),
    /// Hash of the expression structure
    expr_hash: u64,
}

/// View parameter for batch operations
///
/// SAFETY: This struct uses raw pointers to World and must only be used
/// within the scope of a system execution.
#[pyclass(name = "View", frozen)]
#[derive(Clone)]
pub struct PyView {
    /// Component types accessible in this view
    component_types: Vec<PyComponentType>,

    /// Include filter component types (With<T> filters)
    filter_types: Vec<PyComponentType>,

    /// Exclude filter component types (Without<T> filters)
    without_filter_types: Vec<PyComponentType>,

    /// Changed filter component types (Changed<T> filters) - per-entity tick check
    changed_filter_types: Vec<PyComponentType>,

    /// Added filter component types (Added<T> filters) - per-entity tick check
    added_filter_types: Vec<PyComponentType>,

    /// System's last_run tick for change detection comparison
    last_run: Tick,

    /// Current run tick for change detection comparison
    this_run: Tick,

    /// Component types with mutable access (Mut[T] in View parameters)
    /// Components not in this set are read-only
    mutable_components: HashSet<PyComponentType>,

    /// Track which components have already been borrowed mutably
    /// This prevents getting multiple mutable column proxies for the same component
    borrowed_mut: RefCell<HashSet<PyComponentType>>,

    /// Raw pointer to the World (only valid during system execution)
    world_ptr: Option<NonNull<World>>,

    /// Master validity flag - invalidated when system exits
    validity: ValidityFlag,

    /// Primary cache of compiled bytecode (keyed by Python object identity)
    /// Fast path: same Python object always produces same bytecode
    expr_cache: RefCell<HashMap<ExprKey, Arc<CompiledBytecode>>>,

    /// Secondary cache (keyed by expression hash)
    /// Fallback when object identity doesn't match but expression is semantically same
    expr_hash_cache: RefCell<HashMap<ExprHashKey, Arc<CompiledBytecode>>>,

    /// Component ID lookup cache
    component_ids: RefCell<HashMap<PyComponentType, ComponentId>>,

    /// Validity tokens created by iter_batches() that need to be poisoned on drop
    batch_validity_tokens: RefCell<Vec<Arc<std::sync::atomic::AtomicBool>>>,
}

// SAFETY: PyView is only used during system execution on a single thread
unsafe impl Send for PyView {}
unsafe impl Sync for PyView {}

/// Pre-resolved tick filter component IDs for thread-safe parallel access.
///
/// `RefCell`-based `get_component_id()` cannot be called from `par_iter_mut` closures
/// because `RefCell` is not `Sync`. This struct holds the resolved IDs so the parallel
/// closure only needs plain field reads.
struct ResolvedTickFilters {
    changed_ids: Vec<ComponentId>,
    added_ids: Vec<ComponentId>,
    last_run: Tick,
    this_run: Tick,
}

impl ResolvedTickFilters {
    fn entity_passes(&self, entity_mut: &FilteredEntityMut) -> bool {
        if self.changed_ids.is_empty() && self.added_ids.is_empty() {
            return true;
        }

        for &id in &self.changed_ids {
            if let Some(ticks) = entity_mut.get_change_ticks_by_id(id) {
                if !ticks.is_changed(self.last_run, self.this_run) {
                    return false;
                }
            }
        }

        for &id in &self.added_ids {
            if let Some(ticks) = entity_mut.get_change_ticks_by_id(id) {
                if !ticks.is_added(self.last_run, self.this_run) {
                    return false;
                }
            }
        }

        true
    }
}

impl Drop for PyView {
    fn drop(&mut self) {
        // Poison all validity tokens created by iter_batches()
        // This ensures ViewColumn objects become invalid after the system ends
        for token in self.batch_validity_tokens.borrow().iter() {
            token.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

impl PyView {
    /// Create a new View with filter components
    ///
    /// SAFETY: The world pointer must remain valid for the lifetime of this object
    pub unsafe fn new_with_filters(
        component_types: Vec<PyComponentType>,
        mutable_components: HashSet<PyComponentType>,
        filter_types: Vec<PyComponentType>,
        without_filter_types: Vec<PyComponentType>,
        changed_filter_types: Vec<PyComponentType>,
        added_filter_types: Vec<PyComponentType>,
        last_run: Tick,
        world: &mut World,
        validity: ValidityFlag,
    ) -> Self {
        let this_run = world.change_tick();
        Self {
            component_types,
            filter_types,
            without_filter_types,
            changed_filter_types,
            added_filter_types,
            last_run,
            this_run,
            mutable_components,
            borrowed_mut: RefCell::new(HashSet::new()),
            world_ptr: Some(NonNull::from(world)),
            validity,
            expr_cache: RefCell::new(HashMap::new()),
            expr_hash_cache: RefCell::new(HashMap::new()),
            component_ids: RefCell::new(HashMap::new()),
            batch_validity_tokens: RefCell::new(Vec::new()),
        }
    }

    /// Returns true if this view has any per-entity tick filters (Changed or Added)
    fn has_tick_filters(&self) -> bool {
        !self.changed_filter_types.is_empty() || !self.added_filter_types.is_empty()
    }

    /// Build a per-entity tick filter mask for a table.
    ///
    /// Returns a Vec<bool> where `true` means the entity passes all Changed/Added filters.
    /// Returns None if there are no tick filters (all entities pass).
    fn build_tick_mask_for_table(
        &self,
        table: &bevy::ecs::storage::Table,
        entity_count: usize,
    ) -> Option<Vec<bool>> {
        if !self.has_tick_filters() {
            return None;
        }

        let last_run = self.last_run;
        let this_run = self.this_run;
        let mut mask = vec![true; entity_count];

        // Check Changed filters
        for ct in &self.changed_filter_types {
            if let Ok(id) = self.get_component_id(ct) {
                if let Some(column) = table.get_column(id) {
                    let changed_ticks = unsafe { column.get_changed_ticks_slice(entity_count) };
                    for i in 0..entity_count {
                        if mask[i] {
                            let tick = unsafe { *changed_ticks[i].get() };
                            if !tick.is_newer_than(last_run, this_run) {
                                mask[i] = false;
                            }
                        }
                    }
                }
            }
        }

        // Check Added filters
        for ct in &self.added_filter_types {
            if let Ok(id) = self.get_component_id(ct) {
                if let Some(column) = table.get_column(id) {
                    let added_ticks = unsafe { column.get_added_ticks_slice(entity_count) };
                    for i in 0..entity_count {
                        if mask[i] {
                            let tick = unsafe { *added_ticks[i].get() };
                            if !tick.is_newer_than(last_run, this_run) {
                                mask[i] = false;
                            }
                        }
                    }
                }
            }
        }

        Some(mask)
    }

    /// Get or register a component ID
    fn get_component_id(&self, comp_type: &PyComponentType) -> PyResult<ComponentId> {
        // Check cache first
        if let Some(&id) = self.component_ids.borrow().get(comp_type) {
            return Ok(id);
        }

        // Register component
        let world = unsafe {
            self.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("View used outside system execution"))?
                .as_mut()
        };

        let id = register_component_id_simple(world, comp_type);

        // Cache it
        self.component_ids
            .borrow_mut()
            .insert(comp_type.clone(), id);
        Ok(id)
    }

    /// Apply With, Without, Changed, and Added filters to a QueryBuilder.
    /// Changed/Added add `with_id` for archetype filtering; per-entity tick checks happen separately.
    /// Optionally skips a component type (to avoid filtering on the data component itself).
    fn apply_filters_to_query_builder<D: bevy::ecs::query::QueryData>(
        &self,
        builder: &mut QueryBuilder<D>,
        skip: Option<&PyComponentType>,
    ) {
        for filter_type in &self.filter_types {
            if skip.is_some_and(|s| s == filter_type) {
                continue;
            }
            if let Ok(filter_id) = self.get_component_id(filter_type) {
                builder.with_id(filter_id);
            }
        }
        for filter_type in &self.without_filter_types {
            if let Ok(filter_id) = self.get_component_id(filter_type) {
                builder.without_id(filter_id);
            }
        }
        // Changed/Added components need ref_id() for tick access via FilteredEntityMut
        for filter_type in &self.changed_filter_types {
            if skip.is_some_and(|s| s == filter_type) {
                continue;
            }
            if let Ok(filter_id) = self.get_component_id(filter_type) {
                builder.ref_id(filter_id);
            }
        }
        for filter_type in &self.added_filter_types {
            if skip.is_some_and(|s| s == filter_type) {
                continue;
            }
            if let Ok(filter_id) = self.get_component_id(filter_type) {
                builder.ref_id(filter_id);
            }
        }
    }

    /// Pre-resolve tick filter component IDs for thread-safe use in `par_iter_mut`.
    ///
    /// `RefCell<HashMap>` in `get_component_id()` is NOT thread-safe, so we must
    /// resolve all IDs on the main thread before entering parallel iteration.
    fn resolve_tick_filters(&self) -> ResolvedTickFilters {
        let changed_ids = self
            .changed_filter_types
            .iter()
            .filter_map(|ct| self.get_component_id(ct).ok())
            .collect();
        let added_ids = self
            .added_filter_types
            .iter()
            .filter_map(|ct| self.get_component_id(ct).ok())
            .collect();
        ResolvedTickFilters {
            changed_ids,
            added_ids,
            last_run: self.last_run,
            this_run: self.this_run,
        }
    }

    /// Helper: Execute a reduction operation with a custom accumulator function
    fn reduce_with_op<F>(
        &self,
        py: Python<'_>,
        expr: &Bound<'_, PyAny>,
        op: F,
        initial: f64,
    ) -> PyResult<f64>
    where
        F: Fn(f64, f64) -> f64 + Send + Sync,
    {
        let (result, _) = self.reduce_with_count(py, expr, op, initial)?;
        Ok(result)
    }

    /// Helper: Execute a reduction with count tracking
    fn reduce_with_count<F>(
        &self,
        py: Python<'_>,
        expr: &Bound<'_, PyAny>,
        op: F,
        initial: f64,
    ) -> PyResult<(f64, usize)>
    where
        F: Fn(f64, f64) -> f64 + Send + Sync,
    {
        // Compile expression to bytecode
        let rust_expr = RustExpr::from_py_object(py, expr)?;

        // Compile without destination (for reduction, we just evaluate)
        let mut compiler = Compiler::new();
        rust_expr.compile(&mut compiler);
        let bytecode = Arc::new(compiler.finalize());

        // Get world pointer
        let world = unsafe {
            self.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("View used outside system execution"))?
                .as_mut()
        };

        // Determine which component type to query based on the compiled expression.
        // The bytecode's field_map contains the ComponentId for each field reference,
        // so we match it against our registered component IDs.
        let component_type = if bytecode.field_map.is_empty() {
            // Pure constant expression — use first component type
            self.component_types
                .first()
                .ok_or_else(|| PyRuntimeError::new_err("View has no component types"))?
        } else {
            // Find which component type owns the fields referenced in the expression
            let expr_component_id = bytecode.field_map[0].component_id;
            self.component_types
                .iter()
                .find(|ct| {
                    self.get_component_id(ct)
                        .map(|id| id == expr_component_id)
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Expression references component {:?} which is not in the View",
                        expr_component_id
                    ))
                })?
        };

        // Execute reduction based on component type
        // Use generic helper that constrains T::Mutability = Mutable
        let (result, count) = match component_type {
            PyComponentType::Custom(type_ptr) => {
                use std::sync::Mutex;

                use crate::ecs::{component_layout::ComponentStorageType, component_wrapper::*};

                let accumulator = Mutex::new((initial, 0usize));

                // Get Python type and determine storage type
                let storage_type = Python::attach(|py| {
                    let py_type = unsafe {
                        pyo3::Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject)
                    };
                    if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                        ComponentStorageType::from_python_class(&cls)
                            .unwrap_or(ComponentStorageType::PyObject)
                    } else {
                        ComponentStorageType::PyObject
                    }
                });

                match storage_type {
                    ComponentStorageType::Wrapper(wrapper_size) => {
                        // CRITICAL: Query by ComponentId, not by wrapper type!
                        // Multiple custom components can share the same wrapper size,
                        // so we must use the specific component's ComponentId.
                        let component_id = self.get_component_id(component_type)?;

                        let tick_filters = self.resolve_tick_filters();

                        macro_rules! reduce_wrapper {
                            ($wrapper_type:ty) => {{
                                let mut query_builder =
                                    QueryBuilder::<FilteredEntityMut>::new(world);
                                query_builder.mut_id(component_id);

                                self.apply_filters_to_query_builder(
                                    &mut query_builder,
                                    Some(component_type),
                                );

                                let mut query_state = query_builder.build();

                                query_state.par_iter_mut(world).for_each(|mut entity_mut| {
                                    if !tick_filters.entity_passes(&entity_mut) {
                                        return;
                                    }
                                    if let Some(mut untyped) =
                                        entity_mut.get_mut_by_id(component_id)
                                    {
                                        let data_ptr = unsafe {
                                            let wrapper =
                                                untyped.as_mut().deref_mut::<$wrapper_type>();
                                            wrapper.data.as_ptr() as *const u8
                                        };
                                        let value =
                                            self.evaluate_expr_on_wrapper_data(data_ptr, &bytecode);
                                        let mut acc = accumulator.lock().unwrap();
                                        acc.0 = op(acc.0, value);
                                        acc.1 += 1;
                                    }
                                });
                            }};
                        }

                        match wrapper_size {
                            WrapperSize::W8 => reduce_wrapper!(ComponentWrapper8),
                            WrapperSize::W16 => reduce_wrapper!(ComponentWrapper16),
                            WrapperSize::W32 => reduce_wrapper!(ComponentWrapper32),
                            WrapperSize::W64 => reduce_wrapper!(ComponentWrapper64),
                            WrapperSize::W128 => reduce_wrapper!(ComponentWrapper128),
                            WrapperSize::W256 => reduce_wrapper!(ComponentWrapper256),
                            WrapperSize::W512 => reduce_wrapper!(ComponentWrapper512),
                            WrapperSize::W1024 => reduce_wrapper!(ComponentWrapper1024),
                        }
                    }
                    ComponentStorageType::PyObject => {
                        return Err(PyRuntimeError::new_err(
                            "View API not supported for PyObject storage custom components",
                        ));
                    }
                }

                // into_inner only fails on poisoned mutex; par_iter_mut won't
                // poison it since the closure doesn't panic.
                accumulator.into_inner().unwrap()
            }
            PyComponentType::Dynamic(type_ptr) => {
                use std::sync::Mutex;

                use pybevy_core::registry::global_registry;

                let bridge = global_registry::get_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyRuntimeError::new_err("Dynamic component bridge not found"))?;

                let view_bridge = bridge.view_bridge().ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Dynamic component '{}' does not support View reduce (no view_bridge)",
                        bridge.name()
                    ))
                })?;

                // Get component ID via the view_bridge
                let component_id = (view_bridge.component_id)(world);

                let accumulator = Mutex::new((initial, 0usize));

                // Build query using ComponentId
                let mut query_builder = QueryBuilder::<FilteredEntityMut>::new(world);
                query_builder.mut_id(component_id);
                self.apply_filters_to_query_builder(&mut query_builder, Some(component_type));

                let mut query_state = query_builder.build();

                let tick_filters = self.resolve_tick_filters();

                // Execute reduction on all entities in parallel
                query_state.par_iter_mut(world).for_each(|mut entity_mut| {
                    // Per-entity tick filter check
                    if !tick_filters.entity_passes(&entity_mut) {
                        return;
                    }
                    if let Some(mut untyped) = entity_mut.get_mut_by_id(component_id) {
                        // Get raw pointer to component data
                        let ptr = untyped.as_mut().as_ptr() as *const u8;
                        let value = self.evaluate_expr_on_wrapper_data(ptr, &bytecode);
                        let mut acc = accumulator.lock().unwrap();
                        acc.0 = op(acc.0, value);
                        acc.1 += 1;
                    }
                });

                // into_inner only fails on poisoned mutex; par_iter_mut won't
                // poison it since the closure doesn't panic.
                accumulator.into_inner().unwrap()
            }
        };

        Ok((result, count))
    }

    /// Evaluate expression on wrapper storage data (read-only)
    #[inline]
    fn evaluate_expr_on_wrapper_data(
        &self,
        data_ptr: *const u8,
        bytecode: &CompiledBytecode,
    ) -> f64 {
        // Create VM for this execution
        let mut vm = VM::new();

        // Build field pointers for this wrapper (stack-allocated for ≤8 fields)
        let mut field_ptrs: FieldPtrVec = SmallVec::with_capacity(bytecode.field_map.len());
        for field_id in &bytecode.field_map {
            let field_ptr = unsafe { data_ptr.add(field_id.offset) as *mut u8 };
            field_ptrs.push(field_ptr);
        }

        // Use data pointer as entity seed
        let entity_seed = data_ptr as usize;

        // Execute bytecode and get result
        unsafe { vm.execute_and_reduce(bytecode, field_ptrs.as_slice(), entity_seed) }
    }
}

#[pymethods]
impl PyView {
    /// Enable generic syntax: View[Transform] or View[Mut[Transform], Cube]
    ///
    /// Returns a PyViewParam object that encodes the component types and their mutability.
    /// This is similar to how Query[Transform] returns a PyQueryParam.
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        construct_view_class_item(cls, key)
    }

    /// Get a read-only column proxy for a component type
    ///
    /// ```python
    /// vel = view.column(Velocity)
    /// ```
    fn column<'py>(
        &self,
        py: Python<'py>,
        component_type: &Bound<'py, PyType>,
    ) -> PyResult<Py<PyViewCol>> {
        // Verify this type is in the view's component list
        let comp_type = PyComponentType::try_from((component_type, py))?;

        if !self.component_types.contains(&comp_type) {
            return Err(PyTypeError::new_err(format!(
                "Component type {} not in View parameters",
                component_type.name()?
            )));
        }

        let component_id = self.get_component_id(&comp_type)?;

        let proxy = PyViewCol {
            view_ptr: self as *const PyView,
            component_type: comp_type,
            component_id,
            validity: self.validity.clone(),
        };

        Py::new(py, proxy)
    }

    /// Get a mutable column proxy for a component type
    ///
    /// ```python
    /// pos = view.column_mut(Position)
    /// pos.x = pos.x + 1.0  # Triggers compilation and execution
    /// ```
    fn column_mut<'py>(
        &self,
        py: Python<'py>,
        component_type: &Bound<'py, PyType>,
    ) -> PyResult<Py<PyViewColMut>> {
        // Verify this type is in the view's component list AND is mutable
        let comp_type = PyComponentType::try_from((component_type, py))?;

        if !self.component_types.contains(&comp_type) {
            return Err(PyTypeError::new_err(format!(
                "Component type {} not in View parameters",
                component_type.name()?
            )));
        }

        // NEW: Verify component was declared as mutable (Mut[T])
        if !self.mutable_components.contains(&comp_type) {
            return Err(PyRuntimeError::new_err(format!(
                "Component type {} requires mutable access but was not declared with Mut[{}]. Use View[Mut[{}]] in the system signature.",
                component_type.name()?,
                component_type.name()?,
                component_type.name()?
            )));
        }

        // Check if this component has already been borrowed mutably
        {
            let borrowed = self.borrowed_mut.borrow();
            if borrowed.contains(&comp_type) {
                return Err(PyRuntimeError::new_err(format!(
                    "Component type {} already has a mutable column borrowed. Cannot get multiple mutable columns for the same component.",
                    component_type.name()?
                )));
            }
        }

        // Record that we've borrowed this component mutably
        self.borrowed_mut.borrow_mut().insert(comp_type.clone());

        let component_id = self.get_component_id(&comp_type)?;

        let proxy = PyViewColMut {
            view_ptr: self as *const PyView,
            component_type: comp_type,
            component_id,
            validity: self.validity.clone(),
        };

        Py::new(py, proxy)
    }

    /// Reduce operation: Sum all values of an expression across entities
    ///
    /// ```python
    /// total_health = view.reduce_sum(transform.translation.x)
    /// ```
    fn reduce_sum(&self, py: Python<'_>, expr: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.reduce_with_op(py, expr, |acc, val| acc + val, 0.0)
    }

    /// Reduce operation: Compute mean (average) of an expression across entities
    ///
    /// ```python
    /// avg_position = view.reduce_mean(transform.translation.x)
    /// ```
    fn reduce_mean(&self, py: Python<'_>, expr: &Bound<'_, PyAny>) -> PyResult<f64> {
        let (sum, count) = self.reduce_with_count(py, expr, |acc, val| acc + val, 0.0)?;
        if count == 0 {
            Ok(0.0)
        } else {
            Ok(sum / count as f64)
        }
    }

    /// Reduce operation: Find maximum value of an expression across entities
    ///
    /// ```python
    /// max_score = view.reduce_max(transform.scale.z)
    /// ```
    fn reduce_max(&self, py: Python<'_>, expr: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.reduce_with_op(py, expr, |acc, val| acc.max(val), f64::NEG_INFINITY)
    }

    /// Reduce operation: Find minimum value of an expression across entities
    ///
    /// ```python
    /// min_distance = view.reduce_min(distance_expr)
    /// ```
    fn reduce_min(&self, py: Python<'_>, expr: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.reduce_with_op(py, expr, |acc, val| acc.min(val), f64::INFINITY)
    }

    /// Iterate over batches (archetypes) for zero-copy ViewColumn access.
    ///
    /// Each batch represents entities from a single archetype, enabling
    /// zero-copy access via ViewColumn handles that can only be used with
    /// Numba JIT functions.
    ///
    /// ```python
    /// import numba
    ///
    /// @numba.jit(nopython=True)
    /// def add_one(view: ViewColumn):
    ///     for i in range(len(view)):
    ///         view[i] = view[i] + 1.0
    ///
    /// def system(view: View[Mut[Transform]]):
    ///     for batch in view.iter_batches():
    ///         col = batch.column_mut(Transform)
    ///         add_one(col)  # Zero-copy access!
    /// ```
    fn iter_batches(&self, py: Python) -> PyResult<Py<PyBatchIterator>> {
        // Create a new validity token for this batch iteration
        // This token will be poisoned when PyView is dropped (system ends)
        let validity_token = Arc::new(std::sync::atomic::AtomicBool::new(true));

        // Store the token so we can poison it on drop
        self.batch_validity_tokens
            .borrow_mut()
            .push(validity_token.clone());

        // Discover matching archetype tables (same pattern as expression path)
        let world = unsafe {
            self.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("View used outside system execution"))?
                .as_ref()
        };

        // Register all component IDs
        let component_ids: Vec<ComponentId> = self
            .component_types
            .iter()
            .map(|ct| self.get_component_id(ct))
            .collect::<PyResult<Vec<_>>>()?;

        // Register all filter IDs
        let with_filter_ids: Vec<ComponentId> = self
            .filter_types
            .iter()
            .map(|ft| self.get_component_id(ft))
            .collect::<PyResult<Vec<_>>>()?;

        let without_filter_ids: Vec<ComponentId> = self
            .without_filter_types
            .iter()
            .map(|ft| self.get_component_id(ft))
            .collect::<PyResult<Vec<_>>>()?;

        // Changed/Added filter components must also be present on the archetype
        let changed_filter_ids: Vec<ComponentId> = self
            .changed_filter_types
            .iter()
            .map(|ft| self.get_component_id(ft))
            .collect::<PyResult<Vec<_>>>()?;

        let added_filter_ids: Vec<ComponentId> = self
            .added_filter_types
            .iter()
            .map(|ft| self.get_component_id(ft))
            .collect::<PyResult<Vec<_>>>()?;

        // Collect table IDs for all archetypes containing ALL required components AND With filters,
        // and NONE of the Without filter components
        let table_ids: Vec<TableId> = {
            let archetypes = world.archetypes();
            let storages = world.storages();
            let tables = &storages.tables;

            archetypes
                .iter()
                .filter_map(|archetype| {
                    if !component_ids.iter().all(|id| archetype.contains(*id)) {
                        return None;
                    }
                    if !with_filter_ids.iter().all(|id| archetype.contains(*id)) {
                        return None;
                    }
                    if without_filter_ids.iter().any(|id| archetype.contains(*id)) {
                        return None;
                    }
                    // Changed/Added filter components must be present
                    if !changed_filter_ids.iter().all(|id| archetype.contains(*id)) {
                        return None;
                    }
                    if !added_filter_ids.iter().all(|id| archetype.contains(*id)) {
                        return None;
                    }
                    let table_id = archetype.table_id();
                    if tables.get(table_id).map_or(false, |t| t.entity_count() > 0) {
                        Some(table_id)
                    } else {
                        None
                    }
                })
                .collect()
        };

        let total_batches = table_ids.len();

        let iterator = PyBatchIterator {
            component_types: self.component_types.clone(),
            mutable_components: self.mutable_components.clone(),
            world_ptr: self.world_ptr,
            validity_token,
            validity: self.validity.clone(),
            table_ids,
            current_batch: 0,
            total_batches,
        };

        Py::new(py, iterator)
    }

    /// Count entities (optionally matching a boolean condition)
    ///
    /// ```python
    /// low_health_count = view.reduce_count(health < 20.0)
    /// total_count = view.reduce_count()  # Count all entities
    /// ```
    #[pyo3(signature = (expr=None))]
    fn reduce_count(&self, py: Python<'_>, expr: Option<&Bound<'_, PyAny>>) -> PyResult<usize> {
        if let Some(expr_obj) = expr {
            // Count entities where expression evaluates to true (>= 0.5)
            let (sum, _) = self.reduce_with_count(
                py,
                expr_obj,
                |acc, val| {
                    // Treat any value >= 0.5 as true
                    if val >= 0.5 { acc + 1.0 } else { acc }
                },
                0.0,
            )?;
            Ok(sum as usize)
        } else {
            // Count all entities - simpler path without expression evaluation
            let world = unsafe {
                self.world_ptr
                    .ok_or_else(|| PyRuntimeError::new_err("View used outside system execution"))?
                    .as_mut()
            };

            // Build a query using the first component type's ID
            let component_type = self
                .component_types
                .first()
                .ok_or_else(|| PyRuntimeError::new_err("View has no component types"))?;

            let component_id = self.get_component_id(component_type)?;

            let mut query_builder = QueryBuilder::<FilteredEntityMut>::new(world);
            query_builder.mut_id(component_id);
            self.apply_filters_to_query_builder(&mut query_builder, None);

            let mut query_state = query_builder.build();

            // Just count - no expression evaluation needed
            use std::sync::atomic::{AtomicUsize, Ordering};
            let counter = AtomicUsize::new(0);

            let tick_filters = self.resolve_tick_filters();

            query_state.par_iter_mut(world).for_each(|entity_mut| {
                if !tick_filters.entity_passes(&entity_mut) {
                    return;
                }
                counter.fetch_add(1, Ordering::Relaxed);
            });

            Ok(counter.load(Ordering::Relaxed))
        }
    }
}

/// Get field offset and type information for a component field
///
/// For custom components, returns VM FieldType (F32, I64, etc.)
/// For built-in/dynamic components, returns F32 (all built-in fields are f32 or composite of f32)
pub(crate) fn get_component_field_info(
    component_type: &PyComponentType,
    field_name: &str,
) -> PyResult<(usize, pybevy_bytecodevm::bytecode::FieldType)> {
    use pybevy_bytecodevm::bytecode::FieldType as VmFieldType;

    match component_type {
        PyComponentType::Custom(type_ptr) => {
            use crate::ecs::component_layout::{
                ComponentLayout, ComponentStorageType, PrimitiveType,
            };

            // Get Python type and ComponentLayout
            Python::attach(|py| {
                let py_type = unsafe {
                    pyo3::Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject)
                };

                if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                    // Check storage type
                    let storage_type = ComponentStorageType::from_python_class(&cls)
                        .unwrap_or(ComponentStorageType::PyObject);

                    if let ComponentStorageType::Wrapper(_) = storage_type {
                        // Get ComponentLayout for field offsets
                        if let Ok(layout) = ComponentLayout::from_annotations(&cls) {
                            // Handle nested fields (e.g., "position.x" for Vec3 fields)
                            if let Some((base, sub)) = field_name.split_once('.') {
                                for field in &layout.fields {
                                    if field.name == base {
                                        match field.field_type {
                                            PrimitiveType::Vec3 => {
                                                let sub_offset = match sub {
                                                    "x" => 0,
                                                    "y" => 4,
                                                    "z" => 8,
                                                    _ => {
                                                        return Err(PyAttributeError::new_err(
                                                            format!(
                                                                "Vec3 has no sub-field '{}'",
                                                                sub
                                                            ),
                                                        ));
                                                    }
                                                };
                                                return Ok((
                                                    field.offset + sub_offset,
                                                    VmFieldType::F32,
                                                ));
                                            }
                                            PrimitiveType::Vec2 => {
                                                let sub_offset = match sub {
                                                    "x" => 0,
                                                    "y" => 4,
                                                    _ => {
                                                        return Err(PyAttributeError::new_err(
                                                            format!(
                                                                "Vec2 has no sub-field '{}'",
                                                                sub
                                                            ),
                                                        ));
                                                    }
                                                };
                                                return Ok((
                                                    field.offset + sub_offset,
                                                    VmFieldType::F32,
                                                ));
                                            }
                                            _ => {
                                                return Err(PyAttributeError::new_err(format!(
                                                    "Field '{}' is not a composite type",
                                                    base
                                                )));
                                            }
                                        }
                                    }
                                }
                                let available: Vec<&str> =
                                    layout.fields.iter().map(|f| f.name.as_str()).collect();
                                return Err(PyAttributeError::new_err(format!(
                                    "Custom component has no field '{}' (available: {})",
                                    base,
                                    available.join(", ")
                                )));
                            }

                            // Find the field in the layout
                            for field in &layout.fields {
                                if field.name == field_name {
                                    let vm_field_type = field.field_type.to_field_type();
                                    return Ok((field.offset, vm_field_type));
                                }
                            }

                            // Field not found
                            let available: Vec<&str> =
                                layout.fields.iter().map(|f| f.name.as_str()).collect();
                            Err(PyAttributeError::new_err(format!(
                                "Custom component has no field '{}' (available: {})",
                                field_name,
                                available.join(", ")
                            )))
                        } else {
                            Err(PyRuntimeError::new_err(
                                "Failed to get ComponentLayout for custom component",
                            ))
                        }
                    } else {
                        Err(PyTypeError::new_err(
                            "View API only supports wrapper storage custom components",
                        ))
                    }
                } else {
                    Err(PyTypeError::new_err("Invalid custom component type"))
                }
            })
        }
        PyComponentType::Dynamic(type_ptr) => {
            use pybevy_core::registry::global_registry;

            // Get bridge from global registry
            let bridge = global_registry::get_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                PyRuntimeError::new_err("Dynamic component bridge not found in registry")
            })?;

            // Get view bridge for field offset lookup
            let view_bridge = bridge.view_bridge().ok_or_else(|| {
                PyTypeError::new_err(format!(
                    "Component '{}' does not support View API (no view_bridge)",
                    bridge.name()
                ))
            })?;

            // Handle nested fields (e.g., "translation.x" for Transform)
            if let Some((base, sub)) = field_name.split_once('.') {
                // Get base field offset
                let base_offset = (view_bridge.field_offset)(base).ok_or_else(|| {
                    let available = (view_bridge.field_names)().join(", ");
                    PyAttributeError::new_err(format!(
                        "{} has no field '{}' (available: {})",
                        bridge.name(),
                        base,
                        available
                    ))
                })?;

                // Add sub-field offset (x/y/z/w within Vec3/Quat)
                let sub_offset = match sub {
                    "x" => 0,  // First f32
                    "y" => 4,  // Second f32
                    "z" => 8,  // Third f32
                    "w" => 12, // Fourth f32 (for Quat)
                    _ => {
                        return Err(PyTypeError::new_err(format!(
                            "Vec3/Quat has no sub-field '{}'",
                            sub
                        )));
                    }
                };

                return Ok((base_offset.offset + sub_offset, VmFieldType::F32));
            }

            // Top-level field lookup
            let offset_info = (view_bridge.field_offset)(field_name).ok_or_else(|| {
                let available = (view_bridge.field_names)().join(", ");
                PyAttributeError::new_err(format!(
                    "{} has no field '{}' (available: {})",
                    bridge.name(),
                    field_name,
                    available
                ))
            })?;

            // Map FieldOffset dtype to VM FieldType
            let vm_field_type = match offset_info.dtype {
                "u1" => VmFieldType::Bool,
                "f4" => VmFieldType::F32,
                "f8" => VmFieldType::F64,
                "i4" => VmFieldType::I32,
                "i8" => VmFieldType::I64,
                "u4" => VmFieldType::U32,
                "u8" => VmFieldType::U64,
                _ => VmFieldType::F32,
            };

            Ok((offset_info.offset, vm_field_type))
        }
    }
}

/// Shared __getattr__ implementation for both read-only and mutable column proxies
/// Returns the appropriate field proxy (FieldExpr, Vec3Expr, or QuatExpr)
fn create_field_proxy<'py>(
    py: Python<'py>,
    component_type: &PyComponentType,
    component_id: ComponentId,
    field_name: &str,
) -> PyResult<Py<PyAny>> {
    // Import the proxy classes from Python
    let expr_module = py.import("pybevy.expr")?;

    // Determine field offset and type based on component type and field name
    let (offset, field_type) = get_component_field_info(component_type, field_name)?;
    let field_type_str = format!("{:?}", field_type); // FieldType Debug prints as "F32", "I64", etc.

    // For Vec3/Vec2 fields (any component, including custom), return composite proxy
    match field_type {
        pybevy_bytecodevm::bytecode::FieldType::Vec3 => {
            let vec3_proxy = expr_module.getattr("Vec3Expr")?;
            let args = (component_id.index(), field_name, offset);
            let result = vec3_proxy.call1(args)?;
            return Ok(result.unbind());
        }
        pybevy_bytecodevm::bytecode::FieldType::Vec2 => {
            let vec2_proxy = expr_module.getattr("Vec2Expr")?;
            let args = (component_id.index(), field_name, offset);
            let result = vec2_proxy.call1(args)?;
            return Ok(result.unbind());
        }
        _ => {}
    }

    // Check if this is a Transform-like component (Dynamic Transform bridge)
    // Transform fields return F32 from bridge, but we need composite proxies
    let is_transform = match component_type {
        PyComponentType::Dynamic(type_ptr) => {
            use pybevy_core::registry::global_registry;
            global_registry::get_bridge_by_py_type(*type_ptr)
                .map(|b| b.name() == "Transform")
                .unwrap_or(false)
        }
        _ => false,
    };

    // For Transform, translation/scale are Vec3, rotation is Quat
    if is_transform {
        match field_name {
            "translation" | "scale" => {
                let vec3_proxy = expr_module.getattr("Vec3Expr")?;
                let args = (component_id.index(), field_name, offset);
                let result = vec3_proxy.call1(args)?;
                return Ok(result.unbind());
            }
            "rotation" => {
                let quat_proxy = expr_module.getattr("QuatExpr")?;
                let args = (component_id.index(), field_name, offset);
                let result = quat_proxy.call1(args)?;
                return Ok(result.unbind());
            }
            _ => {}
        }
    }

    // For scalar fields, return FieldExpr directly with field type
    let lazy_field_proxy = expr_module.getattr("FieldExpr")?;
    let args = (component_id.index(), field_name, offset, field_type_str);
    let result = lazy_field_proxy.call1(args)?;
    Ok(result.unbind())
}

/// Read-only column proxy for component fields
#[pyclass(name = "ViewCol", frozen)]
pub struct PyViewCol {
    #[allow(dead_code)] // retained for unsafe deref in column operations
    view_ptr: *const PyView,
    component_type: PyComponentType,
    component_id: ComponentId,
    validity: ValidityFlag,
}

// SAFETY: PyViewCol is Send because:
// - The raw pointer is protected by the ValidityFlag (Arc<AtomicBool>)
// - ValidityFlag::check() ensures the pointer is only dereferenced when valid
// - ComponentId and PyComponentType are both Send
unsafe impl Send for PyViewCol {}

// SAFETY: PyViewCol is Sync because:
// - Access to the underlying View is controlled by validity checking
// - The ValidityFlag uses atomic operations for thread-safe access
// - We only allow access when the validity flag is true (during system execution)
unsafe impl Sync for PyViewCol {}

#[pymethods]
impl PyViewCol {
    /// Get the component ID for this column
    #[getter]
    fn component_id(&self) -> u32 {
        self.component_id.index() as u32
    }

    /// Access a field on the component (e.g., `vel.x`)
    ///
    /// Returns a FieldExpr for scalar fields, or Vec3Expr/QuatExpr
    /// for composite fields that can be further accessed.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Py<PyAny>> {
        self.validity.check()?;
        create_field_proxy(py, &self.component_type, self.component_id, name)
    }
}

/// Mutable column proxy for component fields
#[pyclass(name = "ViewColMut", frozen)]
#[derive(Clone)]
pub struct PyViewColMut {
    view_ptr: *const PyView,
    component_type: PyComponentType,
    component_id: ComponentId,
    validity: ValidityFlag,
}

unsafe impl Send for PyViewColMut {}
unsafe impl Sync for PyViewColMut {}

#[pymethods]
impl PyViewColMut {
    /// Get the component ID for this column
    #[getter]
    fn component_id(&self) -> u32 {
        self.component_id.index() as u32
    }

    /// Access a field on the component (e.g., `pos.x`)
    ///
    /// Returns a FieldExpr for scalar fields, or Vec3Expr/QuatExpr
    /// for composite fields that can be further accessed.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Py<PyAny>> {
        self.validity.check()?;

        // Get field proxy using shared helper
        let result = create_field_proxy(py, &self.component_type, self.component_id, name)?;

        // For mutable proxies, set parent reference so field assignments work
        let self_clone = self.clone();
        result.bind(py).call_method1("_set_parent", (self_clone,))?;

        Ok(result)
    }

    /// Set a field value using an expression (the JIT trigger)
    ///
    /// This is called when Python executes: `pos.x = expr`
    ///
    /// ```python
    /// pos.x = pos.x + vel.x * dt  # Triggers this method
    /// ```
    fn __setattr__(&self, py: Python, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self._trigger_assignment(py, name, value)
    }

    /// Trigger field assignment with expression compilation and batch execution.
    ///
    /// Called by `__setattr__` for direct assignments (e.g., `pos.x = expr`)
    /// and by Vec3Expr/QuatExpr for nested assignments (e.g., `pos.translation.y = expr`).
    ///
    /// Uses a two-level cache:
    /// 1. Primary: Python object identity (fastest - no parsing needed)
    /// 2. Secondary: Expression hash (fallback for recreated expressions)
    #[pyo3(name = "_trigger_assignment")]
    fn _trigger_assignment(
        &self,
        py: Python,
        field_name: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.validity.check()?;

        // Get field offset and type for destination
        let (dest_offset, dest_field_type) =
            get_component_field_info(&self.component_type, field_name)?;

        // Get view reference
        let view = unsafe { &*self.view_ptr };

        // OPTIMIZATION: First try identity-based cache (no parsing needed!)
        let expr_identity = value.as_ptr() as usize;
        let identity_key = ExprKey {
            dest_field: (self.component_id, dest_offset),
            expr_identity,
        };

        // Check identity cache first
        {
            let cache = view.expr_cache.borrow();
            if let Some(cached) = cache.get(&identity_key) {
                // Fast path: same Python object, skip parsing entirely
                return self.execute_batch(py, cached);
            }
        }

        // Cache miss - need to parse the expression
        let expr = RustExpr::from_py_object(py, value)?;

        // Compute hash for secondary cache
        let expr_hash = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            format!("{:?}", expr).hash(&mut hasher);
            hasher.finish()
        };

        let hash_key = ExprHashKey {
            dest_field: (self.component_id, dest_offset),
            expr_hash,
        };

        // Check hash cache
        let bytecode = {
            let hash_cache = view.expr_hash_cache.borrow();
            if let Some(cached) = hash_cache.get(&hash_key) {
                // Found in hash cache - also add to identity cache for next time
                let arc_compiled = cached.clone();
                drop(hash_cache); // Release borrow before taking mutable borrow

                let mut identity_cache = view.expr_cache.borrow_mut();
                identity_cache.insert(identity_key, arc_compiled.clone());

                arc_compiled
            } else {
                drop(hash_cache); // Release borrow

                // Full cache miss - compile the expression
                let dest_field = FieldId {
                    component_id: self.component_id,
                    offset: dest_offset,
                    field_type: dest_field_type,
                };

                let compiled = RustExpr::compile_assignment(dest_field, &expr)?;
                let arc_compiled = Arc::new(compiled);

                // Store in both caches
                let mut identity_cache = view.expr_cache.borrow_mut();
                let mut hash_cache = view.expr_hash_cache.borrow_mut();

                identity_cache.insert(identity_key, arc_compiled.clone());
                hash_cache.insert(hash_key, arc_compiled.clone());

                arc_compiled
            }
        };

        // Execute the bytecode on all matching entities
        self.execute_batch(py, &bytecode)?;

        Ok(())
    }
}

impl PyViewColMut {
    /// Execute compiled bytecode on all entities in a parallel batch
    fn execute_batch(&self, _py: Python, bytecode: &CompiledBytecode) -> PyResult<()> {
        // Collect all unique component IDs from the bytecode
        let mut component_ids: HashSet<ComponentId> = HashSet::new();
        for field_id in &bytecode.field_map {
            component_ids.insert(field_id.component_id);
        }

        // Get world reference
        let view = unsafe { &*self.view_ptr };
        let world = unsafe {
            view.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("View used outside system execution"))?
                .as_mut()
        };

        // If all fields are from the same component, use the optimized single-component path
        if component_ids.len() == 1 && component_ids.contains(&self.component_id) {
            return self.execute_batch_single_component(world, view, bytecode);
        }

        // Cross-component expression - use dynamic query
        self.execute_batch_multi_component(world, view, bytecode, component_ids)
    }

    /// Execute bytecode on a single component (optimized path)
    fn execute_batch_single_component(
        &self,
        world: &mut World,
        view: &PyView,
        bytecode: &CompiledBytecode,
    ) -> PyResult<()> {
        // Execute based on component type using concrete types
        match &self.component_type {
            PyComponentType::Dynamic(type_ptr) => {
                // Dynamic component - use ViewBridge from ComponentBridge
                use pybevy_core::registry::global_registry;

                let bridge = global_registry::get_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyRuntimeError::new_err("Dynamic component bridge not found"))?;

                let view_bridge = bridge.view_bridge().ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Dynamic component '{}' does not support View batch execution (no view_bridge)",
                        bridge.name()
                    ))
                })?;

                // Get component ID via the view_bridge
                let component_id = (view_bridge.component_id)(world);

                // Build query using ComponentId (like Custom path does)
                let mut query_builder = QueryBuilder::<FilteredEntityMut>::new(world);
                query_builder.mut_id(component_id);
                view.apply_filters_to_query_builder(&mut query_builder, Some(&self.component_type));

                let mut query_state = query_builder.build();

                let tick_filters = view.resolve_tick_filters();

                // Execute on all entities in parallel
                query_state.par_iter_mut(world).for_each(|mut entity_mut| {
                    // Per-entity tick filter check
                    if !tick_filters.entity_passes(&entity_mut) {
                        return;
                    }
                    if let Some(mut untyped) = entity_mut.get_mut_by_id(component_id) {
                        // Get raw pointer to component data
                        let ptr = untyped.as_mut().as_ptr() as *mut u8;
                        self.execute_on_wrapper_data(ptr, bytecode);
                    }
                });

                Ok(())
            }
            PyComponentType::Custom(type_ptr) => {
                use crate::ecs::{component_layout::ComponentStorageType, component_wrapper::*};

                // Get Python type and determine storage type
                let storage_type = Python::attach(|py| {
                    let py_type = unsafe {
                        pyo3::Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject)
                    };
                    if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                        ComponentStorageType::from_python_class(&cls)
                            .unwrap_or(ComponentStorageType::PyObject)
                    } else {
                        ComponentStorageType::PyObject
                    }
                });

                match storage_type {
                    ComponentStorageType::Wrapper(wrapper_size) => {
                        let tick_filters = view.resolve_tick_filters();

                        // Execute on wrapper storage
                        macro_rules! execute_wrapper {
                            ($wrapper_type:ty) => {{
                                // Get the component ID that was registered for this custom component
                                let component_id = view.get_component_id(&self.component_type)?;

                                // CRITICAL: Query by ComponentId, not by wrapper type!
                                // Multiple custom components can share the same wrapper size
                                let mut query_builder =
                                    QueryBuilder::<FilteredEntityMut>::new(world);
                                query_builder.mut_id(component_id);

                                view.apply_filters_to_query_builder(&mut query_builder, Some(&self.component_type));

                                let mut query_state = query_builder.build();

                                // Execute on all entities in parallel
                                query_state.par_iter_mut(world).for_each(|mut entity_mut| {
                                    // Per-entity tick filter check
                                    if !tick_filters.entity_passes(&entity_mut) {
                                        return;
                                    }
                                    // Get the wrapper by ComponentId
                                    if let Some(mut untyped) = entity_mut.get_mut_by_id(component_id) {
                                        // Cast to wrapper type and execute
                                        unsafe {
                                            let wrapper = untyped.as_mut().deref_mut::<$wrapper_type>();
                                            let data_ptr = wrapper.data.as_mut_ptr() as *mut u8;
                                            self.execute_on_wrapper_data(data_ptr, bytecode);
                                        }
                                    }
                                });

                                Ok(())
                            }};
                        }

                        match wrapper_size {
                            WrapperSize::W8 => execute_wrapper!(ComponentWrapper8),
                            WrapperSize::W16 => execute_wrapper!(ComponentWrapper16),
                            WrapperSize::W32 => execute_wrapper!(ComponentWrapper32),
                            WrapperSize::W64 => execute_wrapper!(ComponentWrapper64),
                            WrapperSize::W128 => execute_wrapper!(ComponentWrapper128),
                            WrapperSize::W256 => execute_wrapper!(ComponentWrapper256),
                            WrapperSize::W512 => execute_wrapper!(ComponentWrapper512),
                            WrapperSize::W1024 => execute_wrapper!(ComponentWrapper1024),
                        }
                    }
                    ComponentStorageType::PyObject => Err(PyRuntimeError::new_err(
                        "View API not supported for PyObject storage custom components",
                    )),
                }
            }
        }
    }

    /// Execute bytecode on multiple components (cross-component expressions)
    ///
    /// Uses batch execution with table iteration for performance.
    fn execute_batch_multi_component(
        &self,
        world: &mut World,
        view: &PyView,
        bytecode: &CompiledBytecode,
        component_ids: HashSet<ComponentId>,
    ) -> PyResult<()> {
        // Get stride for each component
        let components = world.components();
        let mut component_strides: HashMap<ComponentId, usize> = HashMap::new();
        for &component_id in &component_ids {
            let component_info = components.get_info(component_id).ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Component {:?} not found",
                    component_id
                ))
            })?;
            component_strides.insert(component_id, component_info.layout().size());
        }

        // Get filter component IDs
        let with_filter_ids: Vec<ComponentId> = view
            .filter_types
            .iter()
            .filter_map(|ft| view.get_component_id(ft).ok())
            .collect();

        let without_filter_ids: Vec<ComponentId> = view
            .without_filter_types
            .iter()
            .filter_map(|ft| view.get_component_id(ft).ok())
            .collect();

        // Also collect Changed/Added filter IDs for archetype-level presence checks
        let changed_filter_ids: Vec<ComponentId> = view
            .changed_filter_types
            .iter()
            .filter_map(|ft| view.get_component_id(ft).ok())
            .collect();

        let added_filter_ids: Vec<ComponentId> = view
            .added_filter_types
            .iter()
            .filter_map(|ft| view.get_component_id(ft).ok())
            .collect();

        let has_tick_filters = view.has_tick_filters();

        // Collect table info for all archetypes containing ALL required components AND With filters,
        // and NONE of the Without filter components
        struct TableBatch {
            component_bases: HashMap<ComponentId, *mut u8>,
            entity_count: usize,
            /// Per-entity tick mask (None = all pass, Some = per-entity filter)
            tick_mask: Option<Vec<bool>>,
        }

        let table_batches: Vec<TableBatch> = {
            let archetypes = world.archetypes();
            let storages = world.storages();
            let tables = &storages.tables;

            let mut batches = Vec::new();
            for archetype in archetypes.iter() {
                // Skip if archetype doesn't have ALL required components
                if !component_ids.iter().all(|id| archetype.contains(*id)) {
                    continue;
                }

                // Skip if archetype doesn't have ALL With filter components
                if !with_filter_ids.iter().all(|id| archetype.contains(*id)) {
                    continue;
                }

                // Skip if archetype has ANY Without filter components
                if without_filter_ids.iter().any(|id| archetype.contains(*id)) {
                    continue;
                }

                // Changed/Added filter components must be present on archetype
                if !changed_filter_ids.iter().all(|id| archetype.contains(*id)) {
                    continue;
                }
                if !added_filter_ids.iter().all(|id| archetype.contains(*id)) {
                    continue;
                }

                let table_id = archetype.table_id();
                if let Some(table) = tables.get(table_id) {
                    let entity_count = table.entity_count() as usize;
                    if entity_count > 0 {
                        // Build tick mask if needed
                        let tick_mask = if has_tick_filters {
                            view.build_tick_mask_for_table(table, entity_count)
                        } else {
                            None
                        };

                        // Skip entire table if tick mask filters out all entities
                        if let Some(ref mask) = tick_mask {
                            if !mask.iter().any(|&v| v) {
                                continue;
                            }
                        }

                        // Get base pointer for each component
                        let mut component_bases: HashMap<ComponentId, *mut u8> = HashMap::new();
                        let mut all_found = true;

                        for &component_id in &component_ids {
                            if let Some(column) = table.get_column(component_id) {
                                // Get raw pointer to column data
                                // SAFETY: We're inside a mutable world access
                                // Note: get_data_slice expects element count, not byte count
                                // We just need the pointer, stride-based access is handled separately
                                let ptr = unsafe {
                                    let data_slice = column.get_data_slice::<u8>(entity_count);
                                    data_slice.as_ptr() as *mut u8
                                };
                                component_bases.insert(component_id, ptr);
                            } else {
                                all_found = false;
                                break;
                            }
                        }

                        if all_found {
                            batches.push(TableBatch {
                                component_bases,
                                entity_count,
                                tick_mask,
                            });
                        }
                    }
                }
            }
            batches
        };

        if has_tick_filters {
            // Tick-filtered path: process entities individually (cannot use batch VM)
            use rayon::prelude::*;

            // Pre-compute field strides
            let field_strides: Vec<usize> = bytecode
                .field_map
                .iter()
                .map(|field_id| component_strides[&field_id.component_id])
                .collect();

            // Build work items: (table_batch_idx, entity_idx) for all passing entities
            struct EntityWork {
                field_ptrs: Vec<SendPtr>,
                entity_seed: usize,
            }

            let work_items: Vec<EntityWork> = table_batches
                .iter()
                .flat_map(|batch| {
                    let mask = batch.tick_mask.as_ref();
                    let strides = &field_strides;
                    (0..batch.entity_count).filter_map(move |entity_idx| {
                        if let Some(mask) = mask {
                            if !mask[entity_idx] {
                                return None;
                            }
                        }

                        let field_ptrs: Vec<SendPtr> = bytecode
                            .field_map
                            .iter()
                            .enumerate()
                            .map(|(i, field_id)| {
                                let base = batch.component_bases[&field_id.component_id];
                                let stride = strides[i];
                                SendPtr(unsafe {
                                    base.add(field_id.offset).add(entity_idx * stride)
                                })
                            })
                            .collect();

                        Some(EntityWork {
                            field_ptrs,
                            entity_seed: entity_idx,
                        })
                    })
                })
                .collect();

            // Process in parallel
            work_items.par_iter().for_each(|work| {
                let mut vm = VM::new();
                let ptrs: Vec<*mut u8> = work.field_ptrs.iter().map(|p| p.0).collect();
                unsafe {
                    vm.execute(bytecode, &ptrs, work.entity_seed);
                }
            });
        } else {
            // Fast path: no tick filters, batch execution
            use rayon::prelude::*;

            // Build per-field bases and strides based on bytecode field_map
            // For each table, we need to compute:
            // - field_bases[i] = component_base + field_offset
            // - field_strides[i] = component_stride
            struct ChunkInfo {
                field_bases: Vec<SendPtr>,
                field_strides: Vec<usize>,
                count: usize,
            }

            const CHUNK_SIZE: usize = 32768;

            let chunks: Vec<ChunkInfo> = table_batches
                .iter()
                .flat_map(|batch| {
                    // Pre-compute field info for this table
                    let field_strides: Vec<usize> = bytecode
                        .field_map
                        .iter()
                        .map(|field_id| component_strides[&field_id.component_id])
                        .collect();

                    // Split into chunks
                    (0..batch.entity_count)
                        .step_by(CHUNK_SIZE)
                        .map(move |start| {
                            let chunk_count = (batch.entity_count - start).min(CHUNK_SIZE);

                            // Calculate field bases for this chunk
                            let field_bases: Vec<SendPtr> = bytecode
                                .field_map
                                .iter()
                                .enumerate()
                                .map(|(i, field_id)| {
                                    let component_base =
                                        batch.component_bases[&field_id.component_id];
                                    let stride = field_strides[i];
                                    // Base for this chunk = component_base + field_offset + start * stride
                                    let ptr = unsafe {
                                        component_base.add(field_id.offset).add(start * stride)
                                    };
                                    SendPtr(ptr)
                                })
                                .collect();

                            ChunkInfo {
                                field_bases,
                                field_strides: field_strides.clone(),
                                count: chunk_count,
                            }
                        })
                })
                .collect();

            // Process chunks in parallel
            chunks.par_iter().for_each(|chunk| {
                let mut vm = VM::new();
                let bases: Vec<*mut u8> = chunk.field_bases.iter().map(|p| p.0).collect();
                unsafe {
                    vm.execute_batch_multi(bytecode, &bases, &chunk.field_strides, chunk.count);
                }
            });
        }

        // Mark destination component as changed for Bevy's change detection.
        // Raw pointer writes above bypass DerefMut, so systems using Changed<T>
        // (like transform propagation) won't see updates without this.
        let change_tick = world.change_tick();
        let dest_component_id = self.component_id;
        {
            let archetypes = world.archetypes();
            let storages = world.storages();
            let tables = &storages.tables;

            for archetype in archetypes.iter() {
                if !component_ids.iter().all(|id| archetype.contains(*id)) {
                    continue;
                }
                if !with_filter_ids.iter().all(|id| archetype.contains(*id)) {
                    continue;
                }
                if without_filter_ids.iter().any(|id| archetype.contains(*id)) {
                    continue;
                }

                let table_id = archetype.table_id();
                if let Some(table) = tables.get(table_id) {
                    let entity_count = table.entity_count() as usize;
                    if entity_count > 0 {
                        if let Some(column) = table.get_column(dest_component_id) {
                            if has_tick_filters {
                                // Only mark entities that passed tick filters as changed
                                if let Some(mask) =
                                    view.build_tick_mask_for_table(table, entity_count)
                                {
                                    let changed_ticks =
                                        unsafe { column.get_changed_ticks_slice(entity_count) };
                                    for i in 0..entity_count {
                                        if mask[i] {
                                            unsafe {
                                                *changed_ticks[i].get() = change_tick;
                                            }
                                        }
                                    }
                                }
                            } else {
                                let changed_ticks =
                                    unsafe { column.get_changed_ticks_slice(entity_count) };
                                for i in 0..entity_count {
                                    unsafe {
                                        *changed_ticks[i].get() = change_tick;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute bytecode on wrapper storage data
    #[inline]
    fn execute_on_wrapper_data(&self, data_ptr: *mut u8, bytecode: &CompiledBytecode) {
        // Create VM for this execution
        let mut vm = VM::new();

        // Build field pointers for this wrapper (stack-allocated for ≤8 fields)
        let mut field_ptrs: FieldPtrVec = SmallVec::with_capacity(bytecode.field_map.len());
        for field_id in &bytecode.field_map {
            let field_ptr = unsafe { data_ptr.add(field_id.offset) };
            field_ptrs.push(field_ptr);
        }

        // Use data pointer as entity seed for deterministic randomness
        let entity_seed = data_ptr as usize;

        // Execute bytecode
        unsafe {
            vm.execute(bytecode, field_ptrs.as_slice(), entity_seed);
        }
    }
}

/// A batch of entities from a single archetype with zero-copy column access.
///
/// PyBatch provides access to contiguous component storage within a single
/// archetype, enabling zero-copy ViewColumn creation for Numba JIT kernels.
#[pyclass(name = "Batch", frozen)]
pub struct PyBatch {
    /// Component types accessible in this batch
    component_types: Vec<PyComponentType>,

    /// Mutable component types (same as parent View)
    mutable_components: HashSet<PyComponentType>,

    /// Raw pointer to the World
    world_ptr: Option<NonNull<World>>,

    /// Validity token shared with ViewColumn instances
    /// When this is poisoned, all ViewColumns become invalid
    validity_token: Arc<std::sync::atomic::AtomicBool>,

    /// Master validity flag from parent View
    /// Must be checked before dereferencing world_ptr to prevent use-after-free
    validity: ValidityFlag,

    /// Specific archetype table for this batch
    table_id: TableId,
}

unsafe impl Send for PyBatch {}
unsafe impl Sync for PyBatch {}

impl PyBatch {
    /// Create a new batch for zero-copy column access.
    pub(crate) fn new(
        component_types: Vec<PyComponentType>,
        mutable_components: HashSet<PyComponentType>,
        world_ptr: Option<NonNull<World>>,
        validity_token: Arc<std::sync::atomic::AtomicBool>,
        validity: ValidityFlag,
        table_id: TableId,
    ) -> Self {
        Self {
            component_types,
            mutable_components,
            world_ptr,
            validity_token,
            validity,
            table_id,
        }
    }

    /// Get component ID for a component type
    fn get_component_id(&self, comp_type: &PyComponentType) -> PyResult<ComponentId> {
        self.validity.check()?;
        let world = unsafe {
            self.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("Batch not properly initialized"))?
                .as_mut()
        };

        Ok(register_component_id_simple(world, comp_type))
    }
}

#[pymethods]
impl PyBatch {
    /// Get a read-only ViewColumn for a component type.
    ///
    /// Returns an opaque ViewColumn handle that can only be accessed through
    /// Numba JIT functions.
    ///
    /// ```python
    /// for batch in view.iter_batches():
    ///     vel = batch.column(Velocity)  # Read-only ViewColumn
    /// ```
    fn column(&self, py: Python, component_type: &Bound<'_, PyType>) -> PyResult<Py<PyViewColumn>> {
        // Verify this type is in the batch's component list
        let comp_type = PyComponentType::try_from((component_type, py))?;

        if !self.component_types.contains(&comp_type) {
            return Err(PyTypeError::new_err(format!(
                "Component type {} not in View parameters",
                component_type.name()?
            )));
        }

        let component_id = self.get_component_id(&comp_type)?;

        // Access the table directly via stored table_id (archetype filtering already done in iter_batches)
        let world = unsafe {
            self.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("Batch used outside system execution"))?
                .as_ref()
        };

        let storages = world.storages();
        let tables = &storages.tables;
        let table = tables
            .get(self.table_id)
            .ok_or_else(|| PyRuntimeError::new_err("Table not found"))?;

        let entity_count = table.entity_count() as usize;

        let column = table.get_column(component_id).ok_or_else(|| {
            PyRuntimeError::new_err(format!("Column not found for component {:?}", comp_type))
        })?;

        // Get layout from world's component registry
        let components = world.components();
        let component_info = components.get_info(component_id).ok_or_else(|| {
            PyRuntimeError::new_err(format!("Component info not found for {:?}", comp_type))
        })?;
        let layout = component_info.layout();
        let stride = layout.size();

        // Get pointer to column data - match on component type
        let ptr = match comp_type {
            PyComponentType::Custom(_type_ptr) => Python::attach(|py| {
                let py_type = unsafe {
                    pyo3::Bound::from_borrowed_ptr(py, _type_ptr as *mut pyo3::ffi::PyObject)
                };

                if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                    let storage_type = ComponentStorageType::from_python_class(&cls)
                        .unwrap_or(ComponentStorageType::PyObject);

                    match storage_type {
                        ComponentStorageType::Wrapper(wrapper_size) => {
                            let ptr =
                                unsafe { wrapper_size.get_column_data_ptr(column, entity_count) };
                            Ok(ptr)
                        }
                        _ => Err(PyRuntimeError::new_err(
                            "Custom component must use wrapper storage for View API",
                        )),
                    }
                } else {
                    Err(PyRuntimeError::new_err("Invalid component type"))
                }
            })?,
            PyComponentType::Dynamic(type_ptr) => {
                use pybevy_core::registry::global_registry;

                let bridge = global_registry::get_bridge_by_py_type(type_ptr)
                    .ok_or_else(|| PyRuntimeError::new_err("Dynamic component bridge not found"))?;

                let view_bridge = bridge.view_bridge().ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Dynamic component '{}' does not support View column access (no view_bridge)",
                        bridge.name()
                    ))
                })?;

                (view_bridge.column_data_ptr)(column, entity_count)
            }
        };

        let dtype = "struct".to_string();

        let view_column = unsafe {
            match comp_type {
                PyComponentType::Custom(type_ptr) => PyViewColumn::from_raw_parts_with_type(
                    ptr,
                    entity_count,
                    stride,
                    dtype,
                    self.validity_token.clone(),
                    type_ptr,
                ),
                PyComponentType::Dynamic(_) => PyViewColumn::from_raw_parts_with_builtin_type(
                    ptr,
                    entity_count,
                    stride,
                    dtype,
                    self.validity_token.clone(),
                    comp_type,
                ),
            }
        };

        Py::new(py, view_column)
    }

    /// Get a mutable ViewColumn for a component type.
    ///
    /// Returns an opaque ViewColumn handle that can only be accessed through
    /// Numba JIT functions. Requires the component to be declared with Mut[T]
    /// in the View signature.
    ///
    /// ```python
    /// for batch in view.iter_batches():
    ///     pos = batch.column_mut(Position)  # Mutable ViewColumn
    /// ```
    fn column_mut(
        &self,
        py: Python,
        component_type: &Bound<'_, PyType>,
    ) -> PyResult<Py<PyViewColumn>> {
        // Verify this type is in the batch's component list AND is mutable
        let comp_type = PyComponentType::try_from((component_type, py))?;

        if !self.component_types.contains(&comp_type) {
            return Err(PyTypeError::new_err(format!(
                "Component type {} not in View parameters",
                component_type.name()?
            )));
        }

        // Verify component was declared as mutable (Mut[T])
        if !self.mutable_components.contains(&comp_type) {
            return Err(PyRuntimeError::new_err(format!(
                "Component type {} requires mutable access but was not declared with Mut[{}]. Use View[Mut[{}]] in the system signature.",
                component_type.name()?,
                component_type.name()?,
                component_type.name()?
            )));
        }

        let component_id = self.get_component_id(&comp_type)?;

        // Access the table directly via stored table_id (archetype filtering already done in iter_batches)
        // Mutable access needed for change tick marking
        let world = unsafe {
            self.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("Batch used outside system execution"))?
                .as_mut()
        };

        // Get change tick before immutable borrows
        let change_tick = world.change_tick();

        let storages = world.storages();
        let tables = &storages.tables;
        let table = tables
            .get(self.table_id)
            .ok_or_else(|| PyRuntimeError::new_err("Table not found"))?;

        let entity_count = table.entity_count() as usize;

        let column = table.get_column(component_id).ok_or_else(|| {
            PyRuntimeError::new_err(format!("Column not found for component {:?}", comp_type))
        })?;

        // Get layout from world's component registry
        let components = world.components();
        let component_info = components.get_info(component_id).ok_or_else(|| {
            PyRuntimeError::new_err(format!("Component info not found for {:?}", comp_type))
        })?;
        let layout = component_info.layout();
        let stride = layout.size();

        // Mark all entities as changed for Bevy's change detection
        let changed_ticks = unsafe { column.get_changed_ticks_slice(entity_count) };
        for i in 0..entity_count {
            unsafe {
                *changed_ticks[i].get() = change_tick;
            }
        }

        // Get pointer to column data
        let ptr = match comp_type {
            PyComponentType::Custom(_type_ptr) => Python::attach(|py| {
                let py_type = unsafe {
                    pyo3::Bound::from_borrowed_ptr(py, _type_ptr as *mut pyo3::ffi::PyObject)
                };

                if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                    let storage_type = ComponentStorageType::from_python_class(&cls)
                        .unwrap_or(ComponentStorageType::PyObject);

                    match storage_type {
                        ComponentStorageType::Wrapper(wrapper_size) => {
                            let ptr =
                                unsafe { wrapper_size.get_column_data_ptr(column, entity_count) };
                            Ok(ptr)
                        }
                        _ => Err(PyRuntimeError::new_err(
                            "Custom component must use wrapper storage for View API",
                        )),
                    }
                } else {
                    Err(PyRuntimeError::new_err("Invalid component type"))
                }
            })?,
            PyComponentType::Dynamic(type_ptr) => {
                use pybevy_core::registry::global_registry;

                let bridge = global_registry::get_bridge_by_py_type(type_ptr)
                    .ok_or_else(|| PyRuntimeError::new_err("Dynamic component bridge not found"))?;

                let view_bridge = bridge.view_bridge().ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Dynamic component '{}' does not support View column_mut access (no view_bridge)",
                        bridge.name()
                    ))
                })?;

                (view_bridge.column_data_ptr)(column, entity_count)
            }
        };

        let dtype = "struct".to_string();

        let view_column = unsafe {
            match comp_type {
                PyComponentType::Custom(type_ptr) => PyViewColumn::from_raw_parts_with_type(
                    ptr,
                    entity_count,
                    stride,
                    dtype,
                    self.validity_token.clone(),
                    type_ptr,
                ),
                PyComponentType::Dynamic(_) => PyViewColumn::from_raw_parts_with_builtin_type(
                    ptr,
                    entity_count,
                    stride,
                    dtype,
                    self.validity_token.clone(),
                    comp_type,
                ),
            }
        };

        Py::new(py, view_column)
    }

    /// Get the entity IDs for this batch, in the same order as column data.
    ///
    /// ```python
    /// for batch in view.iter_batches():
    ///     entities = batch.entities()
    ///     col = batch.column(Transform)
    ///     # entities[i] corresponds to col data at index i
    /// ```
    fn entities(&self, py: Python) -> PyResult<Vec<Py<PyEntity>>> {
        self.validity.check()?;
        let world = unsafe {
            self.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("Batch used outside system execution"))?
                .as_ref()
        };

        let table = world
            .storages()
            .tables
            .get(self.table_id)
            .ok_or_else(|| PyRuntimeError::new_err("Table not found"))?;

        table
            .entities()
            .iter()
            .map(|&e| Py::new(py, PyEntity::from(e)))
            .collect()
    }

    fn __len__(&self) -> PyResult<usize> {
        self.validity.check()?;
        let world = unsafe {
            self.world_ptr
                .ok_or_else(|| PyRuntimeError::new_err("Batch used outside system execution"))?
                .as_ref()
        };

        let table = world
            .storages()
            .tables
            .get(self.table_id)
            .ok_or_else(|| PyRuntimeError::new_err("Table not found"))?;

        Ok(table.entity_count() as usize)
    }

    fn __repr__(&self) -> String {
        format!(
            "Batch(components={}, valid={})",
            self.component_types.len(),
            self.validity_token
                .load(std::sync::atomic::Ordering::Relaxed)
        )
    }
}

/// Iterator over batches (archetypes) in a View.
#[pyclass(name = "BatchIterator")]
pub struct PyBatchIterator {
    /// Parent view's component types
    component_types: Vec<PyComponentType>,

    /// Mutable components
    mutable_components: HashSet<PyComponentType>,

    /// World pointer
    world_ptr: Option<NonNull<World>>,

    /// Validity token for all batches
    validity_token: Arc<std::sync::atomic::AtomicBool>,

    /// Master validity flag
    validity: ValidityFlag,

    /// Table IDs for each matching archetype
    table_ids: Vec<TableId>,

    /// Current batch index
    current_batch: usize,

    /// Total number of batches
    total_batches: usize,
}

unsafe impl Send for PyBatchIterator {}
unsafe impl Sync for PyBatchIterator {}

#[pymethods]
impl PyBatchIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python) -> PyResult<Option<Py<PyBatch>>> {
        self.validity.check()?;

        if self.current_batch >= self.total_batches {
            return Ok(None);
        }

        let table_id = self.table_ids[self.current_batch];
        self.current_batch += 1;

        let batch = PyBatch::new(
            self.component_types.clone(),
            self.mutable_components.clone(),
            self.world_ptr,
            self.validity_token.clone(),
            self.validity.clone(),
            table_id,
        );

        Ok(Some(Py::new(py, batch)?))
    }
}
