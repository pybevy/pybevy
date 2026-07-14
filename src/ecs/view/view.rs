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
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        change_detection::Tick,
        component::ComponentId,
        query::QueryBuilder,
        world::{FilteredEntityMut, World, unsafe_world_cell::UnsafeWorldCell},
    },
    prelude::*,
};
use pybevy_bytecodevm::{
    bytecode::{CompiledBytecode, Compiler, FieldType as VmFieldType},
    expr::RustExpr,
    view_engine::{self, TableRowRange, ViewFilter},
    view_runtime::{ViewRuntimeCore, ViewRuntimeError},
};
use pybevy_core::{PyEntity, registry::global_registry};
use pyo3::{
    exceptions::{PyAttributeError, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyType},
};

use crate::ecs::{
    component_layout::{
        ComponentLayout, ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt,
        PrimitiveType, PrimitiveTypeExt,
    },
    component_type::{PyComponentType, register_component_id_simple},
    component_wrapper::*,
    helpers::validity_guard::ValidityFlag,
    view::{cached_view::CachedPyView, construct_view_class_item, view_column::PyViewColumn},
};

/// View parameter for batch operations
///
/// SAFETY: This struct uses raw pointers to World and must only be used
/// within the scope of a system execution.
#[pyclass(name = "View", frozen, skip_from_py_object)]
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

    /// Stable interpreter adapter and run-scoped neutral runtime.
    cached: Arc<CachedPyView>,
    runtime: Arc<ViewRuntimeCore>,

    /// Component types with mutable access (Mut[T] in View parameters)
    /// Components not in this set are read-only
    mutable_components: HashSet<PyComponentType>,

    /// Track which components have already been borrowed mutably
    /// This prevents getting multiple mutable column proxies for the same component
    borrowed_mut: RefCell<HashSet<PyComponentType>>,

    /// World cell (lifetime-erased), valid only during system execution. The View's
    /// batch ops (QueryBuilder / par_iter_mut) fundamentally need `&mut World`, so a
    /// `*mut World` is derived per-operation from this cell (see `world_ptr`).
    world_cell: Option<UnsafeWorldCell<'static>>,

    /// Master validity flag - invalidated when system exits
    validity: ValidityFlag,

    /// Bytecode cache for compiled expressions (keyed by dest field + expression hash)
    bytecode_cache: RefCell<view_engine::BytecodeCache>,

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
            if let Some(ticks) = entity_mut.get_change_ticks_by_id(id)
                && !ticks.is_changed(self.last_run, self.this_run)
            {
                return false;
            }
        }

        for &id in &self.added_ids {
            if let Some(ticks) = entity_mut.get_change_ticks_by_id(id)
                && !ticks.is_added(self.last_run, self.this_run)
            {
                return false;
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
    /// # Safety
    /// `world` must reference the World the view operates on and must remain valid
    /// for as long as `validity` is active.
    pub unsafe fn new_cached(
        cached: Arc<CachedPyView>,
        last_run: Tick,
        this_run: Tick,
        world: UnsafeWorldCell,
        validity: ValidityFlag,
    ) -> Result<Self, ViewRuntimeError> {
        // SAFETY: forwarded from the caller: the cell and validity belong to
        // this run, while cached metadata was built from the same declared View.
        let runtime = Arc::new(unsafe {
            ViewRuntimeCore::new(
                Arc::clone(&cached.core),
                world,
                validity.clone(),
                last_run,
                this_run,
            )
        }?);
        // SAFETY: layout-preserving lifetime erasure of a Copy pointer type; the cell
        // is only used while `validity` is active.
        let world_cell: UnsafeWorldCell<'static> = unsafe { std::mem::transmute(world) };
        Ok(Self {
            component_types: cached.component_types.clone(),
            filter_types: cached.filter_types.clone(),
            without_filter_types: cached.without_filter_types.clone(),
            changed_filter_types: cached.changed_filter_types.clone(),
            added_filter_types: cached.added_filter_types.clone(),
            last_run,
            this_run,
            mutable_components: cached.mutable_components.clone(),
            cached,
            runtime,
            borrowed_mut: RefCell::new(HashSet::new()),
            world_cell: Some(world_cell),
            validity,
            bytecode_cache: RefCell::new(view_engine::BytecodeCache::new()),
            batch_validity_tokens: RefCell::new(Vec::new()),
        })
    }

    /// Derive a raw `*mut World` from the stored cell for a single batch operation.
    ///
    /// The View's batch machinery (`QueryBuilder`, `par_iter_mut`) fundamentally
    /// requires `&mut World`, so a pointer is derived per-operation rather than a
    /// long-lived borrow. This is the same residual-pointer pattern as
    /// `query_runtime::world_ptr`.
    ///
    /// SAFETY of dereferencing the returned pointer: `initialize` declares this
    /// view's component read/write access; the executor prevents a conflicting
    /// system from running concurrently, so the data the batch ops touch is unique.
    fn world_ptr(&self) -> PyResult<*mut World> {
        let cell = self
            .world_cell
            .ok_or_else(|| PyRuntimeError::new_err("View used outside system execution"))?;
        // SAFETY: momentary derivation of a Copy pointer; see method docs.
        Ok(unsafe { cell.world_mut() as *mut World })
    }

    /// Build a `ViewFilter` from this view's filter types for use with `view_engine` functions.
    fn build_view_filter(&self, component_ids: HashSet<ComponentId>) -> PyResult<ViewFilter> {
        Ok(ViewFilter {
            component_ids,
            with_ids: self
                .filter_types
                .iter()
                .filter_map(|ft| self.get_component_id(ft).ok())
                .collect(),
            without_ids: self
                .without_filter_types
                .iter()
                .filter_map(|ft| self.get_component_id(ft).ok())
                .collect(),
            changed_ids: self
                .changed_filter_types
                .iter()
                .filter_map(|ft| self.get_component_id(ft).ok())
                .collect(),
            added_ids: self
                .added_filter_types
                .iter()
                .filter_map(|ft| self.get_component_id(ft).ok())
                .collect(),
        })
    }

    /// The set of component ids this view declares as data. Used as the
    /// `allowed` set for `validate_bytecode_components`: a `field` expression
    /// naming a component outside this set (e.g. a proxy captured from another
    /// View) is rejected before its column is read.
    fn declared_component_ids(&self) -> HashSet<ComponentId> {
        self.runtime.spec().filter().component_ids.clone()
    }

    /// The per-component accept-set for `validate_bytecode_field_types`: each
    /// declared component's `ComponentId` mapped to its legitimate bytecode
    /// `(offset, FieldType)` pairs (vectors expanded to `F32` lanes). A `field`
    /// expression whose `(offset, field_type)` is not in its component's set is a
    /// type confusion (or mid-field offset) and is rejected before execution.
    fn declared_field_offsets(&self) -> HashMap<ComponentId, HashSet<(usize, VmFieldType)>> {
        self.runtime.spec().allowed_fields().clone()
    }

    /// Get the initialization-time component ID for this View parameter.
    fn get_component_id(&self, comp_type: &PyComponentType) -> PyResult<ComponentId> {
        self.cached.component_id(comp_type).ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Component type {comp_type} was not resolved for this View parameter"
            ))
        })
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
        // SAFETY: momentary &mut World for a batch op; see PyView::world_ptr.
        let world = unsafe { &mut *self.world_ptr()? };

        // SECURITY: validate field offsets before raw-pointer evaluation
        // (reduction path uses `evaluate_on_ptr` -> `base.add(offset)`).
        view_engine::validate_bytecode_offsets(world, &bytecode)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        // SECURITY: reject fields naming a component this view did not declare
        // (undeclared-read race + would panic on the stride/base lookup).
        view_engine::validate_bytecode_components(&bytecode, &self.declared_component_ids())
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        // SECURITY: reject fields whose (offset, type) don't name a real field
        // (type confusion, e.g. reading an f32 field as Bool = invalid-bit-pattern UB).
        view_engine::validate_bytecode_field_types(&bytecode, &self.declared_field_offsets())
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;

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
                let accumulator = Mutex::new((initial, 0usize));

                // Get Python type and determine storage type
                let storage_type = Python::attach(|py| {
                    // SAFETY: registered type pointers live for the interpreter lifetime
                    let py_type = unsafe {
                        pyo3::Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject)
                    };
                    if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                        ComponentStorageType::from_python_class(cls)
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
                                        // SAFETY: deref_mut + evaluate_on_ptr: pointer from
                                        // get_mut_by_id() with matching wrapper layout.
                                        let value = unsafe {
                                            let wrapper =
                                                untyped.as_mut().deref_mut::<$wrapper_type>();
                                            let data_ptr = wrapper.data.as_ptr() as *const u8;
                                            self.evaluate_expr_on_wrapper_data(data_ptr, &bytecode)
                                        };
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
                        let ptr = untyped.as_mut().as_ptr() as *const u8;
                        // SAFETY: ptr from get_mut_by_id() points to valid component data
                        let value = unsafe { self.evaluate_expr_on_wrapper_data(ptr, &bytecode) };
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

    /// Evaluate expression on wrapper storage data (read-only).
    ///
    /// # Safety
    ///
    /// `data_ptr` must point to valid component data whose layout matches
    /// the field offsets in `bytecode`.
    #[inline]
    unsafe fn evaluate_expr_on_wrapper_data(
        &self,
        data_ptr: *const u8,
        bytecode: &CompiledBytecode,
    ) -> f64 {
        unsafe { view_engine::evaluate_on_ptr(data_ptr, bytecode) }
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

    /// Iterate over contiguous filtered table-row batches for zero-copy ViewColumn access.
    ///
    /// Each batch represents a contiguous range of selected rows from one ECS
    /// table, enabling zero-copy access via ViewColumn handles that can only be
    /// used with Numba JIT functions.
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

        // Discover the exact matching table-row ranges. Sparse-set components
        // can split one table across multiple archetypes, so a table ID alone
        // is not a valid representation of a filtered batch.
        // SAFETY: momentary &World for archetype discovery; see PyView::world_ptr.
        let world = unsafe { &*self.world_ptr()? };

        let component_ids: HashSet<ComponentId> = self
            .component_types
            .iter()
            .map(|ct| self.get_component_id(ct))
            .collect::<PyResult<HashSet<_>>>()?;
        let filter = self.build_view_filter(component_ids)?;
        let table_ranges = view_engine::matching_table_row_ranges(world, &filter);

        let total_batches = table_ranges.len();

        let iterator = PyBatchIterator {
            component_types: self.component_types.clone(),
            mutable_components: self.mutable_components.clone(),
            world_cell: self.world_cell,
            validity_token,
            validity: self.validity.clone(),
            table_ranges,
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
            let lease = self
                .runtime
                .gather_batches()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            lease
                .entity_count()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
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
    match component_type {
        PyComponentType::Custom(type_ptr) => {
            // Get Python type and ComponentLayout
            Python::attach(|py| {
                // SAFETY: registered type pointers live for the interpreter lifetime
                let py_type = unsafe {
                    pyo3::Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject)
                };

                if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                    // Check storage type
                    let storage_type = ComponentStorageType::from_python_class(cls)
                        .unwrap_or(ComponentStorageType::PyObject);

                    if let ComponentStorageType::Wrapper(_) = storage_type {
                        // Get ComponentLayout for field offsets
                        if let Ok(layout) = ComponentLayout::from_annotations(cls) {
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

            Ok((offset_info.offset, offset_info.field_type))
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
    // Vec4 fields fall through to `_` - no Vec4Expr proxy yet
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
        PyComponentType::Dynamic(type_ptr) => global_registry::get_bridge_by_py_type(*type_ptr)
            .map(|b| b.name() == "Transform")
            .unwrap_or(false),
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
#[pyclass(name = "ViewColMut", frozen, skip_from_py_object)]
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
    /// Uses `BytecodeCache` from `view_engine` for frame-persistent caching
    /// keyed by (component_id, field_offset, expression_hash).
    #[pyo3(name = "_trigger_assignment")]
    fn _trigger_assignment(
        &self,
        py: Python,
        field_name: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.validity.check()?;

        let (dest_offset, dest_field_type) =
            get_component_field_info(&self.component_type, field_name)?;

        let view = unsafe { &*self.view_ptr };

        // Parse the Python expression to RustExpr
        let expr = RustExpr::from_py_object(py, value)?;
        let expr_hash = view_engine::BytecodeCache::expr_hash(&expr);

        // Get or compile bytecode (cached across frames)
        let bytecode = view
            .bytecode_cache
            .borrow_mut()
            .get_or_compile(
                self.component_id,
                dest_offset,
                dest_field_type,
                &expr,
                expr_hash,
            )
            .clone();

        self.execute_batch(py, &bytecode)
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
        // SAFETY: momentary &mut World for a batch op; see PyView::world_ptr.
        let world = unsafe { &mut *view.world_ptr()? };

        // SECURITY: validate every field offset (source fields AND the
        // assignment destination, which `compile_assignment` appends to
        // `field_map`) against each component's registered layout before any
        // raw-pointer arithmetic. A Python-constructed `Expr("field", ...)`
        // can supply an arbitrary `offset`; without this check the VM would
        // compute `base.add(offset)` and read/write out of bounds of the
        // component column allocation.
        view_engine::validate_bytecode_offsets(world, bytecode)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        // SECURITY: reject fields naming a component this view did not declare
        // (undeclared-read race on assignment sources + stride/base lookup panic).
        view_engine::validate_bytecode_components(bytecode, &view.declared_component_ids())
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        // SECURITY: reject fields whose (offset, type) don't name a real field
        // (type confusion on assignment source/dest, e.g. Bool over an f32 field = UB).
        view_engine::validate_bytecode_field_types(bytecode, &view.declared_field_offsets())
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;

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
            PyComponentType::Dynamic(_type_ptr) => {
                let filter = view.build_view_filter([self.component_id].into_iter().collect())?;

                view_engine::execute_query_assignment(world, self.component_id, &filter, bytecode);
                Ok(())
            }
            PyComponentType::Custom(type_ptr) => {
                // Get Python type and determine storage type
                let storage_type = Python::attach(|py| {
                    // SAFETY: registered type pointers live for the interpreter lifetime
                    let py_type = unsafe {
                        pyo3::Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject)
                    };
                    if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                        ComponentStorageType::from_python_class(cls)
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
    /// Uses `ViewExecutionContext` for cached batch execution with automatic
    /// change tick marking.
    fn execute_batch_multi_component(
        &self,
        world: &mut World,
        view: &PyView,
        bytecode: &CompiledBytecode,
        component_ids: HashSet<ComponentId>,
    ) -> PyResult<()> {
        let filter = view.build_view_filter(component_ids)?;

        let ctx =
            view_engine::ViewExecutionContext::new(world, &filter, view.last_run, view.this_run)
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))?;

        unsafe {
            ctx.execute(world, bytecode, self.component_id);
        }
        Ok(())
    }

    /// Execute bytecode on wrapper storage data.
    ///
    /// # Safety
    ///
    /// `data_ptr` must point to valid component data whose layout matches
    /// the field offsets in `bytecode`.
    #[inline]
    unsafe fn execute_on_wrapper_data(&self, data_ptr: *mut u8, bytecode: &CompiledBytecode) {
        unsafe { view_engine::execute_on_ptr(data_ptr, bytecode) };
    }
}

/// A contiguous batch of filtered table rows with zero-copy column access.
///
/// PyBatch provides access to a contiguous range of component storage within
/// one table, enabling zero-copy ViewColumn creation for Numba JIT kernels.
#[pyclass(name = "Batch", frozen)]
pub struct PyBatch {
    /// Component types accessible in this batch
    component_types: Vec<PyComponentType>,

    /// Mutable component types (same as parent View)
    mutable_components: HashSet<PyComponentType>,

    /// World cell (lifetime-erased), valid only during system execution. A
    /// `*mut World` is derived per-operation (see `world_ptr`).
    world_cell: Option<UnsafeWorldCell<'static>>,

    /// Validity token shared with ViewColumn instances
    /// When this is poisoned, all ViewColumns become invalid
    validity_token: Arc<std::sync::atomic::AtomicBool>,

    /// Master validity flag from parent View
    /// Must be checked before dereferencing the world cell to prevent use-after-free
    validity: ValidityFlag,

    /// Exact contiguous table-row range selected by the View filters.
    table_range: TableRowRange,
}

unsafe impl Send for PyBatch {}
unsafe impl Sync for PyBatch {}

impl PyBatch {
    /// Create a new batch for zero-copy column access.
    pub(crate) fn new(
        component_types: Vec<PyComponentType>,
        mutable_components: HashSet<PyComponentType>,
        world_cell: Option<UnsafeWorldCell<'static>>,
        validity_token: Arc<std::sync::atomic::AtomicBool>,
        validity: ValidityFlag,
        table_range: TableRowRange,
    ) -> Self {
        Self {
            component_types,
            mutable_components,
            world_cell,
            validity_token,
            validity,
            table_range,
        }
    }

    /// Derive a raw `*mut World` from the stored cell for a single batch operation.
    ///
    /// Same residual-pointer pattern as `PyView::world_ptr`: the batch column and
    /// change-tick machinery need `&mut World`; the parent view's declared
    /// component access bounds the data actually touched.
    fn world_ptr(&self) -> PyResult<*mut World> {
        let cell = self
            .world_cell
            .ok_or_else(|| PyRuntimeError::new_err("Batch not properly initialized"))?;
        // SAFETY: momentary derivation of a Copy pointer; see method docs.
        Ok(unsafe { cell.world_mut() as *mut World })
    }

    /// Get component ID for a component type
    fn get_component_id(&self, comp_type: &PyComponentType) -> PyResult<ComponentId> {
        self.validity.check()?;
        // SAFETY: momentary &mut World for component registration; see world_ptr.
        let world = unsafe { &mut *self.world_ptr()? };

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
        // SAFETY: momentary &World for table access; see PyBatch::world_ptr.
        let world = unsafe { &*self.world_ptr()? };

        let storages = world.storages();
        let tables = &storages.tables;
        let table = tables
            .get(self.table_range.table_id)
            .ok_or_else(|| PyRuntimeError::new_err("Table not found"))?;

        let table_entity_count = table.entity_count() as usize;
        let range_end = self
            .table_range
            .start_row
            .checked_add(self.table_range.entity_count)
            .filter(|&end| end <= table_entity_count)
            .ok_or_else(|| PyRuntimeError::new_err("Batch row range is no longer valid"))?;

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
                // SAFETY: registered type pointers live for the interpreter lifetime
                let py_type = unsafe {
                    pyo3::Bound::from_borrowed_ptr(py, _type_ptr as *mut pyo3::ffi::PyObject)
                };

                if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                    let storage_type = ComponentStorageType::from_python_class(cls)
                        .unwrap_or(ComponentStorageType::PyObject);

                    match storage_type {
                        ComponentStorageType::Wrapper(wrapper_size) => {
                            let ptr = unsafe {
                                wrapper_size.get_column_data_ptr(column, table_entity_count)
                            };
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
                let bridge = global_registry::get_bridge_by_py_type(type_ptr)
                    .ok_or_else(|| PyRuntimeError::new_err("Dynamic component bridge not found"))?;

                let view_bridge = bridge.view_bridge().ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Dynamic component '{}' does not support View column access (no view_bridge)",
                        bridge.name()
                    ))
                })?;

                unsafe { (view_bridge.column_data_ptr)(column, table_entity_count) }
            }
        };
        // SAFETY: the row range was checked against the current table size,
        // and `ptr` addresses the first element of this component column.
        let ptr = unsafe { ptr.add(self.table_range.start_row * stride) };
        let entity_count = range_end - self.table_range.start_row;

        let view_column = unsafe {
            match comp_type {
                PyComponentType::Custom(type_ptr) => PyViewColumn::from_raw_parts_with_type(
                    ptr,
                    entity_count,
                    stride,
                    self.validity_token.clone(),
                    type_ptr,
                ),
                PyComponentType::Dynamic(_) => PyViewColumn::from_raw_parts_with_builtin_type(
                    ptr,
                    entity_count,
                    stride,
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
        // SAFETY: momentary &mut World for change-tick marking; see PyBatch::world_ptr.
        let world = unsafe { &mut *self.world_ptr()? };

        // Get change tick before immutable borrows
        let change_tick = world.change_tick();

        let storages = world.storages();
        let tables = &storages.tables;
        let table = tables
            .get(self.table_range.table_id)
            .ok_or_else(|| PyRuntimeError::new_err("Table not found"))?;

        let table_entity_count = table.entity_count() as usize;
        let range_end = self
            .table_range
            .start_row
            .checked_add(self.table_range.entity_count)
            .filter(|&end| end <= table_entity_count)
            .ok_or_else(|| PyRuntimeError::new_err("Batch row range is no longer valid"))?;

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
        // SAFETY: the column belongs to this table and `table_entity_count` is
        // its current row count. The selected range was bounds-checked above.
        let changed_ticks = unsafe { column.get_changed_ticks_slice(table_entity_count) };
        for tick in &changed_ticks[self.table_range.start_row..range_end] {
            // SAFETY: this tick belongs to the bounds-checked selected range and
            // the parent View declared mutable access to this component.
            unsafe {
                *tick.get() = change_tick;
            }
        }

        // Get pointer to column data
        let ptr = match comp_type {
            PyComponentType::Custom(_type_ptr) => Python::attach(|py| {
                // SAFETY: registered type pointers live for the interpreter lifetime
                let py_type = unsafe {
                    pyo3::Bound::from_borrowed_ptr(py, _type_ptr as *mut pyo3::ffi::PyObject)
                };

                if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                    let storage_type = ComponentStorageType::from_python_class(cls)
                        .unwrap_or(ComponentStorageType::PyObject);

                    match storage_type {
                        ComponentStorageType::Wrapper(wrapper_size) => {
                            let ptr = unsafe {
                                wrapper_size.get_column_data_ptr(column, table_entity_count)
                            };
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
                let bridge = global_registry::get_bridge_by_py_type(type_ptr)
                    .ok_or_else(|| PyRuntimeError::new_err("Dynamic component bridge not found"))?;

                let view_bridge = bridge.view_bridge().ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Dynamic component '{}' does not support View column_mut access (no view_bridge)",
                        bridge.name()
                    ))
                })?;

                unsafe { (view_bridge.column_data_ptr)(column, table_entity_count) }
            }
        };
        // SAFETY: the row range was checked against the current table size,
        // and `ptr` addresses the first element of this component column.
        let ptr = unsafe { ptr.add(self.table_range.start_row * stride) };
        let entity_count = range_end - self.table_range.start_row;

        let view_column = unsafe {
            match comp_type {
                PyComponentType::Custom(type_ptr) => PyViewColumn::from_raw_parts_with_type(
                    ptr,
                    entity_count,
                    stride,
                    self.validity_token.clone(),
                    type_ptr,
                ),
                PyComponentType::Dynamic(_) => PyViewColumn::from_raw_parts_with_builtin_type(
                    ptr,
                    entity_count,
                    stride,
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
        // SAFETY: momentary &World for table access; see PyBatch::world_ptr.
        let world = unsafe { &*self.world_ptr()? };

        let table = world
            .storages()
            .tables
            .get(self.table_range.table_id)
            .ok_or_else(|| PyRuntimeError::new_err("Table not found"))?;

        let range_end = self
            .table_range
            .start_row
            .checked_add(self.table_range.entity_count)
            .filter(|&end| end <= table.entity_count() as usize)
            .ok_or_else(|| PyRuntimeError::new_err("Batch row range is no longer valid"))?;

        table.entities()[self.table_range.start_row..range_end]
            .iter()
            .map(|&e| Py::new(py, PyEntity::from(e)))
            .collect()
    }

    fn __len__(&self) -> PyResult<usize> {
        self.validity.check()?;
        Ok(self.table_range.entity_count)
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

    /// World cell (lifetime-erased), passed to each PyBatch it yields
    world_cell: Option<UnsafeWorldCell<'static>>,

    /// Validity token for all batches
    validity_token: Arc<std::sync::atomic::AtomicBool>,

    /// Master validity flag
    validity: ValidityFlag,

    /// Exact contiguous table-row ranges selected by the View filters.
    table_ranges: Vec<TableRowRange>,

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

        let table_range = self.table_ranges[self.current_batch];
        self.current_batch += 1;

        let batch = PyBatch::new(
            self.component_types.clone(),
            self.mutable_components.clone(),
            self.world_cell,
            self.validity_token.clone(),
            self.validity.clone(),
            table_range,
        );

        Ok(Some(Py::new(py, batch)?))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        mem,
        sync::{Arc, atomic::AtomicBool},
    };

    use bevy::{
        ecs::component::Component,
        prelude::{Entity, Transform},
    };
    use pybevy_bytecodevm::view_engine::{ViewFilter, matching_table_row_ranges};
    use pybevy_core::{ValidityGuard, bridge_inventory};
    use pybevy_transform::transform::PyTransform;
    use pyo3::{PyTypeInfo, types::PyAnyMethods};

    use super::*;

    #[derive(Component)]
    #[component(storage = "SparseSet")]
    struct SparseBatchMarker;

    #[test]
    fn pybatch_binding_limits_columns_entities_and_ticks_to_sparse_selected_range() {
        bridge_inventory::collect_all();

        let mut world = World::new();
        let plain_first = world.spawn(Transform::from_xyz(10.0, 0.0, 0.0)).id();
        let selected_first = world
            .spawn((Transform::from_xyz(20.0, 0.0, 0.0), SparseBatchMarker))
            .id();
        let selected_second = world
            .spawn((Transform::from_xyz(30.0, 0.0, 0.0), SparseBatchMarker))
            .id();
        let plain_last = world.spawn(Transform::from_xyz(40.0, 0.0, 0.0)).id();

        let transform_id = world.components().component_id::<Transform>().unwrap();
        let marker_id = world
            .components()
            .component_id::<SparseBatchMarker>()
            .unwrap();
        let filter = ViewFilter {
            component_ids: HashSet::from([transform_id]),
            with_ids: vec![marker_id],
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        };
        let ranges = matching_table_row_ranges(&world, &filter);
        assert_eq!(ranges.len(), 1);
        let table_range = ranges[0];
        assert_eq!(table_range.start_row, 1);
        assert_eq!(table_range.entity_count, 2);

        world.clear_trackers();
        let last_run = world.last_change_tick();
        world.increment_change_tick();
        let this_run = world.change_tick();

        // SAFETY: `world` outlives the Python batch and is not structurally
        // modified while the batch and its derived column are alive.
        let world_cell: UnsafeWorldCell<'static> =
            unsafe { mem::transmute(world.as_unsafe_world_cell()) };
        let validity = ValidityFlag::new();
        let _validity_guard = ValidityGuard::new(validity.clone());
        let validity_token = Arc::new(AtomicBool::new(true));

        Python::attach(|py| {
            let transform_type = PyTransform::type_object(py);
            let component_type = PyComponentType::Dynamic(transform_type.as_type_ptr());
            let batch = Py::new(
                py,
                PyBatch::new(
                    vec![component_type.clone()],
                    HashSet::from([component_type]),
                    Some(world_cell),
                    validity_token,
                    validity,
                    table_range,
                ),
            )
            .unwrap();
            let batch = batch.bind(py);

            assert_eq!(
                batch
                    .call_method0("__len__")
                    .unwrap()
                    .extract::<usize>()
                    .unwrap(),
                2
            );
            let entities = batch.call_method0("entities").unwrap();
            assert_eq!(entities.len().unwrap(), 2);

            let column = batch.call_method1("column_mut", (transform_type,)).unwrap();
            assert_eq!(
                column.getattr("len").unwrap().extract::<usize>().unwrap(),
                2
            );
            let column = column.cast::<PyViewColumn>().unwrap().borrow();
            let x_column = column
                .at_offset_typed(
                    mem::offset_of!(Transform, translation),
                    Some(VmFieldType::F32),
                )
                .unwrap();
            drop(column);
            let x_column = Py::new(py, x_column).unwrap();
            let x_column = x_column.bind(py);
            assert_eq!(
                x_column
                    .call_method1("peek", (0,))
                    .unwrap()
                    .extract::<f64>()
                    .unwrap(),
                20.0
            );
            assert_eq!(
                x_column
                    .call_method1("peek", (1,))
                    .unwrap()
                    .extract::<f64>()
                    .unwrap(),
                30.0
            );
            x_column.call_method1("set", (99.0,)).unwrap();
        });

        assert_eq!(
            world
                .entity(plain_first)
                .get::<Transform>()
                .unwrap()
                .translation
                .x,
            10.0
        );
        assert_eq!(
            world
                .entity(selected_first)
                .get::<Transform>()
                .unwrap()
                .translation
                .x,
            99.0
        );
        assert_eq!(
            world
                .entity(selected_second)
                .get::<Transform>()
                .unwrap()
                .translation
                .x,
            99.0
        );
        assert_eq!(
            world
                .entity(plain_last)
                .get::<Transform>()
                .unwrap()
                .translation
                .x,
            40.0
        );

        let was_changed = |entity: Entity| {
            world
                .entity(entity)
                .get_change_ticks_by_id(transform_id)
                .unwrap()
                .is_changed(last_run, this_run)
        };
        assert!(!was_changed(plain_first));
        assert!(was_changed(selected_first));
        assert!(was_changed(selected_second));
        assert!(!was_changed(plain_last));
    }
}
