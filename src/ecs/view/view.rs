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
    collections::HashSet,
    mem::size_of,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        change_detection::Tick, component::ComponentId, world::unsafe_world_cell::UnsafeWorldCell,
    },
    prelude::*,
};
use pybevy_bytecodevm::{
    bytecode::{FieldId, FieldType as VmFieldType},
    expr::RustExpr,
    view_runtime::{
        BatchSlice, ViewReduction, ViewReductionOutput, ViewRuntimeCore, ViewRuntimeError,
    },
};
use pybevy_core::{
    FieldType as StorageFieldType, PyEntity, public_error::RESOURCE_VIEW_DATA,
    registry::global_registry,
};
use pyo3::{
    exceptions::{PyAttributeError, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyDict, PyType},
};

use crate::ecs::{
    component_layout::{
        ComponentLayout, ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt,
        PrimitiveType, PrimitiveTypeExt,
    },
    component_type::PyComponentType,
    helpers::validity_guard::ValidityFlag,
    view::{cached_view::CachedPyView, construct_view_class_item, view_column::PyViewColumn},
};

/// View parameter for batch operations
///
/// The run-scoped core owns the World cell and rejects stale or cross-thread
/// access before any pointer operation.
#[pyclass(name = "View", module = "pybevy.ecs", frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct PyView {
    /// Stable interpreter adapter and run-scoped neutral runtime.
    cached: Arc<CachedPyView>,
    runtime: Arc<ViewRuntimeCore>,

    /// Track which components have already been borrowed mutably
    /// This prevents getting multiple mutable column proxies for the same component
    borrowed_mut: Arc<Mutex<HashSet<PyComponentType>>>,

    /// Master validity flag - invalidated when system exits
    validity: ValidityFlag,
}

// SAFETY: moving the Python wrapper cannot access World storage. Every core
// operation checks the run's thread-affine validity and shared operation fence.
unsafe impl Send for PyView {}
// SAFETY: shared wrapper access is subject to the same checks; scheduler access
// encoded by the cached View spec excludes conflicting ECS operations.
unsafe impl Sync for PyView {}

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
        Ok(Self {
            cached,
            runtime,
            borrowed_mut: Arc::new(Mutex::new(HashSet::new())),
            validity,
        })
    }

    /// Get the initialization-time component ID for this View parameter.
    fn get_component_id(
        &self,
        comp_type: &PyComponentType,
        py: Python<'_>,
    ) -> PyResult<ComponentId> {
        self.cached.component_id(comp_type).ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Component type {} was not resolved for this View parameter",
                comp_type.display_name(py)
            ))
        })
    }

    /// Compile, validate, and reduce one expression through the neutral core.
    fn reduce_expression(
        &self,
        py: Python<'_>,
        expr: &Bound<'_, PyAny>,
        reduction: ViewReduction,
    ) -> PyResult<ViewReductionOutput> {
        let expression = RustExpr::from_py_object(py, expr)?;
        let program = self
            .runtime
            .prepare_read_program(&expression)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let lease = self
            .runtime
            .gather_batches()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        lease
            .reduce(&program, reduction, true)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
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

        if !self.cached.component_types.contains(&comp_type) {
            return Err(PyTypeError::new_err(format!(
                "Component type {} not in View parameters",
                component_type.name()?
            )));
        }

        let component_id = self.get_component_id(&comp_type, py)?;

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

        if !self.cached.component_types.contains(&comp_type) {
            return Err(PyTypeError::new_err(format!(
                "Component type {} not in View parameters",
                component_type.name()?
            )));
        }

        // NEW: Verify component was declared as mutable (Mut[T])
        if !self.cached.mutable_components.contains(&comp_type) {
            return Err(PyRuntimeError::new_err(format!(
                "Component type {} requires mutable access but was not declared with Mut[{}]. Use View[Mut[{}]] in the system signature.",
                component_type.name()?,
                component_type.name()?,
                component_type.name()?
            )));
        }

        let has_declared_fields = component_type
            .getattr("__annotations__")
            .ok()
            .and_then(|annotations| annotations.cast_into::<PyDict>().ok())
            .is_some_and(|annotations| !annotations.is_empty());
        if has_declared_fields
            && matches!(&comp_type, PyComponentType::Custom(_))
            && matches!(
                ComponentStorageType::from_python_class(component_type)?,
                ComponentStorageType::PyObject
            )
        {
            return Err(PyRuntimeError::new_err(format!(
                "Component type {} uses Python-object storage, which is not supported by View.column_mut()",
                component_type.name()?
            )));
        }

        // Check and record under one lock so free-threaded callers cannot both
        // acquire a mutable proxy for the same component.
        let inserted = self
            .borrowed_mut
            .lock()
            .map_err(|_| PyRuntimeError::new_err("View mutable-borrow lock was poisoned"))?
            .insert(comp_type);
        if !inserted {
            return Err(PyRuntimeError::new_err(format!(
                "Component type {} already has a mutable column borrowed. Cannot get multiple mutable columns for the same component.",
                component_type.name()?
            )));
        }

        let component_id = self.get_component_id(&comp_type, py)?;

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
        Ok(self.reduce_expression(py, expr, ViewReduction::Sum)?.value)
    }

    /// Reduce operation: Compute mean (average) of an expression across entities
    ///
    /// ```python
    /// avg_position = view.reduce_mean(transform.translation.x)
    /// ```
    fn reduce_mean(&self, py: Python<'_>, expr: &Bound<'_, PyAny>) -> PyResult<f64> {
        let output = self.reduce_expression(py, expr, ViewReduction::Sum)?;
        if output.count == 0 {
            Ok(0.0)
        } else {
            Ok(output.value / output.count as f64)
        }
    }

    /// Reduce operation: Find maximum value of an expression across entities
    ///
    /// ```python
    /// max_score = view.reduce_max(transform.scale.z)
    /// ```
    fn reduce_max(&self, py: Python<'_>, expr: &Bound<'_, PyAny>) -> PyResult<f64> {
        Ok(self.reduce_expression(py, expr, ViewReduction::Max)?.value)
    }

    /// Reduce operation: Find minimum value of an expression across entities
    ///
    /// ```python
    /// min_distance = view.reduce_min(distance_expr)
    /// ```
    fn reduce_min(&self, py: Python<'_>, expr: &Bound<'_, PyAny>) -> PyResult<f64> {
        Ok(self.reduce_expression(py, expr, ViewReduction::Min)?.value)
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
        let lease = Arc::new(
            self.runtime
                .gather_batches()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?,
        );
        let slices = lease
            .contiguous_slices()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;

        let iterator = PyBatchIterator {
            cached: Arc::clone(&self.cached),
            slices,
            current_batch: 0,
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
            Ok(self
                .reduce_expression(py, expr_obj, ViewReduction::CountTruthy)?
                .value as usize)
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

                let lane = match sub {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    "w" => 3,
                    _ => usize::MAX,
                };
                let lane_count = match base_offset.field_type {
                    StorageFieldType::Vec2 => 2,
                    StorageFieldType::Vec3 => 3,
                    StorageFieldType::Vec4 => 4,
                    _ => 0,
                };
                if lane >= lane_count {
                    return Err(PyTypeError::new_err(format!(
                        "field '{base}' of type {:?} has no sub-field '{sub}'",
                        base_offset.field_type
                    )));
                }
                let sub_offset = lane * size_of::<f32>();

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
        PyComponentType::Resource(_) => Err(PyTypeError::new_err(RESOURCE_VIEW_DATA)),
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
#[pyclass(name = "ViewCol", module = "pybevy.ecs", frozen)]
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
#[pyclass(
    name = "ViewColMut",
    module = "pybevy.ecs",
    frozen,
    skip_from_py_object
)]
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
    /// Compiled programs are cached on the stable `CachedViewCore` and always
    /// pass intent/layout/access validation before execution.
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

        // SAFETY: validity was checked immediately above. The proxy and parent
        // are created for the same run and the run guard invalidates the shared
        // flag before the parent can be used again.
        let view = unsafe { &*self.view_ptr };

        // Parse the Python expression to RustExpr
        let expr = RustExpr::from_py_object(py, value)?;
        let destination = FieldId {
            component_id: self.component_id,
            offset: dest_offset,
            field_type: dest_field_type,
        };
        let program = view
            .runtime
            .prepare_assignment_program(destination, &expr)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let lease = view
            .runtime
            .gather_batches()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        lease
            .execute_assignment(&program, true)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }
}

/// A contiguous batch of filtered table rows with zero-copy column access.
///
/// PyBatch provides access to a contiguous range of component storage within
/// one table, enabling zero-copy ViewColumn creation for Numba JIT kernels.
#[pyclass(name = "Batch", module = "pybevy.ecs", frozen)]
pub struct PyBatch {
    cached: Arc<CachedPyView>,
    slice: BatchSlice,
}

// SAFETY: `BatchSlice` retains the core lease and every safe operation checks
// validity/thread affinity and acquires the shared pointer-operation fence.
unsafe impl Send for PyBatch {}
// SAFETY: see `Send`; mutable access is checked against the resolved View spec.
unsafe impl Sync for PyBatch {}

impl PyBatch {
    /// Create a new batch for zero-copy column access.
    pub(crate) fn new(cached: Arc<CachedPyView>, slice: BatchSlice) -> Self {
        Self { cached, slice }
    }

    /// Get component ID for a component type
    fn get_component_id(
        &self,
        comp_type: &PyComponentType,
        py: Python<'_>,
    ) -> PyResult<ComponentId> {
        self.slice
            .check_valid()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        self.cached.component_id(comp_type).ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Component type {} was not resolved for this View parameter",
                comp_type.display_name(py)
            ))
        })
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

        if !self.cached.component_types.contains(&comp_type) {
            return Err(PyTypeError::new_err(format!(
                "Component type {} not in View parameters",
                component_type.name()?
            )));
        }

        let component_id = self.get_component_id(&comp_type, py)?;
        let column = self
            .slice
            .column(component_id, false)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let view_column = match comp_type {
            PyComponentType::Custom(type_ptr) => {
                PyViewColumn::from_batch_column_with_type(column, type_ptr)
            }
            PyComponentType::Dynamic(_) => {
                PyViewColumn::from_batch_column_with_builtin_type(column, comp_type)
            }
            PyComponentType::Resource(_) => {
                unreachable!("resource components are rejected when View parameters are parsed")
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

        if !self.cached.component_types.contains(&comp_type) {
            return Err(PyTypeError::new_err(format!(
                "Component type {} not in View parameters",
                component_type.name()?
            )));
        }

        // Verify component was declared as mutable (Mut[T])
        if !self.cached.mutable_components.contains(&comp_type) {
            return Err(PyRuntimeError::new_err(format!(
                "Component type {} requires mutable access but was not declared with Mut[{}]. Use View[Mut[{}]] in the system signature.",
                component_type.name()?,
                component_type.name()?,
                component_type.name()?
            )));
        }

        let component_id = self.get_component_id(&comp_type, py)?;
        let column = self
            .slice
            .column(component_id, true)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let view_column = match comp_type {
            PyComponentType::Custom(type_ptr) => {
                PyViewColumn::from_batch_column_with_type(column, type_ptr)
            }
            PyComponentType::Dynamic(_) => {
                PyViewColumn::from_batch_column_with_builtin_type(column, comp_type)
            }
            PyComponentType::Resource(_) => {
                unreachable!("resource components are rejected when View parameters are parsed")
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
        self.slice
            .entities()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?
            .into_iter()
            .map(|entity| Py::new(py, PyEntity::from(entity)))
            .collect()
    }

    fn __len__(&self) -> PyResult<usize> {
        self.slice
            .check_valid()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(self.slice.len())
    }

    fn __repr__(&self) -> String {
        format!(
            "Batch(components={}, valid={})",
            self.cached.component_types.len(),
            self.slice.check_valid().is_ok()
        )
    }
}

/// Iterator over exact contiguous filtered batches in a View.
#[pyclass(name = "BatchIterator", module = "pybevy.ecs")]
pub struct PyBatchIterator {
    cached: Arc<CachedPyView>,
    slices: Vec<BatchSlice>,

    /// Current batch index
    current_batch: usize,
}

// SAFETY: slices retain their lease and cannot expose storage without the core
// validity/thread/fence checks.
unsafe impl Send for PyBatchIterator {}
// SAFETY: iteration only clones unforgeable slices; storage access happens in
// `PyBatch` under the same core checks.
unsafe impl Sync for PyBatchIterator {}

#[pymethods]
impl PyBatchIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python) -> PyResult<Option<Py<PyBatch>>> {
        if self.current_batch >= self.slices.len() {
            return Ok(None);
        }

        let slice = self.slices[self.current_batch].clone();
        slice
            .check_valid()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        self.current_batch += 1;
        let batch = PyBatch::new(Arc::clone(&self.cached), slice);

        Ok(Some(Py::new(py, batch)?))
    }
}
