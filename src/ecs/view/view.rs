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
    FieldType as StorageFieldType, PyEntity,
    batch_columns::check_view_assignment,
    component_fields::declared_annotations,
    domain::FieldDomain,
    public_error::{self, RESOURCE_VIEW_DATA, VIEW_COLUMN_READ_ONLY_ASSIGNMENT},
    registry::global_registry,
};
use pyo3::{
    exceptions::{PyAttributeError, PyOverflowError, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyType},
};

use crate::ecs::{
    component_layout::{
        ComponentLayout, ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt,
        PrimitiveType, PrimitiveTypeExt, extract_primitive_from_py,
    },
    component_type::PyComponentType,
    helpers::validity_guard::ValidityFlag,
    view::{cached_view::CachedPyView, construct_view_class_item, view_column::PyViewColumn},
};

fn view_program_error(error: ViewRuntimeError) -> PyErr {
    match error {
        ViewRuntimeError::ImplicitFloatToInteger { .. }
        | ViewRuntimeError::ImplicitSignedToUnsigned { .. } => {
            PyTypeError::new_err(error.to_string())
        }
        ViewRuntimeError::IntegerLiteralOutOfBounds(_) => {
            PyOverflowError::new_err(error.to_string())
        }
        _ => PyValueError::new_err(error.to_string()),
    }
}

fn view_execution_error(error: ViewRuntimeError) -> PyErr {
    match error {
        ViewRuntimeError::NegativeIntegerPower => PyValueError::new_err(error.to_string()),
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

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
            .map_err(view_program_error)?;
        let lease = self
            .runtime
            .gather_batches()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        lease
            .reduce(&program, reduction, true)
            .map_err(view_execution_error)
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

        let has_declared_fields = !declared_annotations(component_type)?.is_empty();
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
    /// @system
    /// def my_system(view: View[Mut[Transform]]):
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

/// A custom component's matrix field has no batch buffer representation.
pub(crate) fn matrix_view_field_error(field: &str, field_type: PrimitiveType) -> PyErr {
    PyTypeError::new_err(public_error::view_matrix_field_unsupported(
        field,
        format!("{field_type:?}"),
    ))
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
        PyComponentType::Custom(type_ptr) => Python::attach(|py| {
            let (offset, primitive) = custom_view_field_info(py, *type_ptr, field_name)?;
            let field_type = primitive
                .to_field_type()
                .ok_or_else(|| matrix_view_field_error(field_name, primitive))?;
            Ok((offset, field_type))
        }),
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

fn ensure_batch_column_supported(
    component_type: &Bound<'_, PyType>,
    resolved_type: &PyComponentType,
) -> PyResult<()> {
    let name = component_type.name()?;
    match resolved_type {
        PyComponentType::Custom(_) => {
            let ComponentStorageType::Wrapper(_) =
                ComponentStorageType::from_python_class(component_type)?
            else {
                return Err(PyTypeError::new_err(format!(
                    "Component type {name} uses Python-object storage, which is not supported by View batches"
                )));
            };
            let layout = ComponentLayout::from_annotations(component_type)?;
            if !layout
                .fields
                .iter()
                .any(|field| field.field_type.to_field_type().is_some())
            {
                return Err(PyTypeError::new_err(format!(
                    "Component type {name} has no View-compatible fields"
                )));
            }
        }
        PyComponentType::Dynamic(type_ptr) => {
            let supports_view = global_registry::get_bridge_by_py_type(*type_ptr)
                .and_then(|bridge| bridge.view_bridge())
                .is_some();
            if !supports_view {
                return Err(PyTypeError::new_err(format!(
                    "Component type {name} does not support View batches"
                )));
            }
        }
        PyComponentType::Resource(_) => unreachable!("resources are rejected in View parameters"),
    }
    Ok(())
}

/// The owner, field and domain that a checked View write to `field_name` must
/// satisfy, or `None` when View writes to it are unchecked.
pub(crate) fn native_view_field_domains(
    component_type: &PyComponentType,
    field_name: &str,
) -> Option<(&'static str, &'static str, &'static [FieldDomain])> {
    let PyComponentType::Dynamic(type_ptr) = component_type else {
        return None;
    };
    let base_field = field_name
        .split_once('.')
        .map_or(field_name, |(base, _)| base);
    global_registry::get_component_batch_meta(*type_ptr as usize)?
        .fields
        .iter()
        .find(|field| field.name == base_field && field.view_checked && !field.domains.is_empty())
        .map(|field| (field.owner, field.name, field.domains))
}

/// Resolve custom field shapes and bounded matrix/vector projections.
fn custom_view_field_info(
    py: Python<'_>,
    type_ptr: *const pyo3::ffi::PyTypeObject,
    field_name: &str,
) -> PyResult<(usize, PrimitiveType)> {
    // SAFETY: registered type pointers live for the interpreter lifetime.
    let py_type = unsafe { Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
    let class = py_type.cast::<PyType>()?;
    if !matches!(
        ComponentStorageType::from_python_class(class)?,
        ComponentStorageType::Wrapper(_)
    ) {
        return Err(PyTypeError::new_err(
            "View API only supports wrapper storage custom components",
        ));
    }
    let layout = ComponentLayout::from_annotations(class)?;
    let (base, sub) = field_name
        .split_once('.')
        .map_or((field_name, None), |(base, sub)| (base, Some(sub)));
    let field = layout.get_field(base).ok_or_else(|| {
        let available: Vec<_> = layout
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect();
        PyAttributeError::new_err(format!(
            "Custom component has no field '{base}' (available: {})",
            available.join(", ")
        ))
    })?;
    let mut primitive = field.field_type;
    let mut offset = field.offset;
    let Some(mut sub) = sub else {
        return Ok((offset, primitive));
    };
    if primitive.is_matrix() {
        let (axis, lane) = sub
            .split_once('.')
            .map_or((sub, None), |(axis, lane)| (axis, Some(lane)));
        let dimension = match primitive {
            PrimitiveType::Mat2 => 2,
            PrimitiveType::Mat3 => 3,
            PrimitiveType::Mat4 => 4,
            _ => unreachable!(),
        };
        let column = ["x_axis", "y_axis", "z_axis", "w_axis"][..dimension]
            .iter()
            .position(|name| *name == axis)
            .ok_or_else(|| {
                PyAttributeError::new_err(format!("{primitive:?} has no sub-field '{axis}'"))
            })?;
        offset += column * dimension * size_of::<f32>();
        primitive = [
            PrimitiveType::Vec2,
            PrimitiveType::Vec3,
            PrimitiveType::Vec4,
        ][dimension - 2];
        let Some(lane) = lane else {
            return Ok((offset, primitive));
        };
        sub = lane;
    }
    if !primitive.is_composite() {
        return Err(PyAttributeError::new_err(format!(
            "Field '{base}' is not a composite type"
        )));
    }
    let lane = ["x", "y", "z", "w"][..primitive.element_count()]
        .iter()
        .position(|name| *name == sub)
        .ok_or_else(|| {
            PyAttributeError::new_err(format!("{primitive:?} has no sub-field '{sub}'"))
        })?;
    Ok((offset + lane * size_of::<f32>(), PrimitiveType::F32))
}

fn lazy_view_field_info(
    py: Python<'_>,
    component_type: &PyComponentType,
    field_name: &str,
) -> PyResult<(usize, VmFieldType, Option<PrimitiveType>)> {
    if let PyComponentType::Custom(type_ptr) = component_type {
        let (offset, primitive) = custom_view_field_info(py, *type_ptr, field_name)?;
        return if primitive.is_composite() {
            Ok((offset, VmFieldType::F32, Some(primitive)))
        } else {
            Ok((
                offset,
                primitive.to_field_type().expect("scalar VM field type"),
                None,
            ))
        };
    }
    let (offset, field_type) = get_component_field_info(component_type, field_name)?;
    let primitive = match field_type {
        VmFieldType::Vec2 => Some(PrimitiveType::Vec2),
        VmFieldType::Vec3 => Some(PrimitiveType::Vec3),
        VmFieldType::Vec4 => {
            if matches!(component_type, PyComponentType::Dynamic(type_ptr)
                if field_name == "rotation" && global_registry::get_bridge_by_py_type(*type_ptr).is_some_and(|bridge| bridge.name() == "Transform"))
            {
                Some(PrimitiveType::Quat)
            } else {
                Some(PrimitiveType::Vec4)
            }
        }
        _ => None,
    };
    Ok((
        offset,
        if primitive.is_some() {
            VmFieldType::F32
        } else {
            field_type
        },
        primitive,
    ))
}

/// Construct the expression proxy for the declared scalar or composite shape.
fn create_field_proxy<'py>(
    py: Python<'py>,
    component_type: &PyComponentType,
    component_id: ComponentId,
    field_name: &str,
) -> PyResult<Py<PyAny>> {
    let module = py.import("pybevy.expr")?;
    let (offset, field_type, primitive) = lazy_view_field_info(py, component_type, field_name)?;
    if let Some(primitive) = primitive {
        let proxy = module.getattr(format!("{primitive:?}Expr"))?;
        Ok(proxy
            .call1((component_id.index(), field_name, offset))?
            .unbind())
    } else {
        let proxy = module.getattr("FieldExpr")?;
        Ok(proxy
            .call1((
                component_id.index(),
                field_name,
                offset,
                format!("{field_type:?}"),
            ))?
            .unbind())
    }
}

/// Extract only matching expression shapes, or a validity-checked native constant.
fn composite_assignment_expressions(
    py: Python<'_>,
    primitive: PrimitiveType,
    value: &Bound<'_, PyAny>,
) -> PyResult<Vec<RustExpr>> {
    let module = py.import("pybevy.expr")?;
    let is_proxy = value.is_instance(&module.getattr(format!("{primitive:?}Expr"))?)?;
    let is_expression = matches!(
        primitive,
        PrimitiveType::Vec2 | PrimitiveType::Vec3 | PrimitiveType::Vec4
    ) && value
        .is_instance(&module.getattr(format!("{primitive:?}Expression"))?)?;
    if is_proxy || is_expression {
        let axes = ["x", "y", "z", "w"];
        let count = primitive.element_count();
        let mut expressions = Vec::with_capacity(count);
        if primitive.is_matrix() {
            let dimension = match primitive {
                PrimitiveType::Mat2 => 2,
                PrimitiveType::Mat3 => 3,
                PrimitiveType::Mat4 => 4,
                _ => unreachable!(),
            };
            for axis in &axes[..dimension] {
                let column = value.getattr(format!("{axis}_axis"))?;
                for lane in &axes[..dimension] {
                    expressions.push(RustExpr::from_py_object(py, &column.getattr(*lane)?)?);
                }
            }
        } else {
            for axis in &axes[..count] {
                expressions.push(RustExpr::from_py_object(py, &value.getattr(*axis)?)?);
            }
        }
        return Ok(expressions);
    }
    let math = py.import("pybevy.math")?;
    if !value.is_instance(&math.getattr(format!("{primitive:?}"))?)? {
        return Err(PyTypeError::new_err(
            public_error::view_composite_assignment_type(format!("{primitive:?}")),
        ));
    }
    let primitive_value = extract_primitive_from_py(primitive, value)?;
    let (lanes, count) = primitive_value
        .f32_lanes()
        .expect("composite f32 representation");
    Ok(lanes[..count]
        .iter()
        .map(|value| RustExpr::Const(f64::from(*value)))
        .collect())
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

    /// Reject writes on the read-only lazy facade.
    ///
    /// `col.a = value` must report the read-only contract instead of falling
    /// through to a missing-attribute error. Validity is checked first so
    /// expired and cross-thread access keeps its precedence.
    fn __setattr__(&self, _name: &str, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.validity.check()?;
        Err(PyRuntimeError::new_err(VIEW_COLUMN_READ_ONLY_ASSIGNMENT))
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

// SAFETY: parent access checks owning-thread validity before dereferencing the run-bound pointer.
unsafe impl Send for PyViewColMut {}
// SAFETY: writes use the parent's validated scheduler access and execution lease.
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

        let (dest_offset, dest_field_type, primitive) =
            lazy_view_field_info(py, &self.component_type, field_name)?;
        let expressions = if let Some(primitive) = primitive {
            composite_assignment_expressions(py, primitive, value)?
        } else {
            vec![RustExpr::from_py_object(py, value)?]
        };

        self.validity.check()?;

        // SAFETY: validity was checked immediately above. The proxy and parent
        // are created for the same run and the run guard invalidates the shared
        // flag before the parent can be used again.
        let view = unsafe { &*self.view_ptr };

        let destination = FieldId {
            component_id: self.component_id,
            offset: dest_offset,
            field_type: dest_field_type,
        };
        let program = if primitive.is_some() {
            view.runtime
                .prepare_composite_assignment_program(destination, &expressions)
        } else {
            view.runtime
                .prepare_assignment_program(destination, &expressions[0])
        }
        .map_err(view_program_error)?;
        let lease = view
            .runtime
            .gather_batches()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        if let Some((owner, field, domains)) =
            native_view_field_domains(&self.component_type, field_name)
        {
            let lane_type = dest_field_type;
            for expression in &expressions {
                let read_program = view
                    .runtime
                    .prepare_read_program(expression)
                    .map_err(view_program_error)?;
                let values = lease
                    .evaluate(&read_program, true)
                    .map_err(view_execution_error)?;
                check_view_assignment(owner, field, &values, lane_type, domains)
                    .map_err(PyErr::from)?;
            }
        }
        lease
            .execute_assignment(&program, true)
            .map_err(view_execution_error)
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
        ensure_batch_column_supported(component_type, &comp_type)?;

        let component_id = self.get_component_id(&comp_type, py)?;
        let column = self
            .slice
            .column(component_id, false)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let view_column = match comp_type {
            PyComponentType::Custom(_) => {
                PyViewColumn::from_batch_column_with_type(column, component_type)
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
        ensure_batch_column_supported(component_type, &comp_type)?;

        let component_id = self.get_component_id(&comp_type, py)?;
        let column = self
            .slice
            .column(component_id, true)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let view_column = match comp_type {
            PyComponentType::Custom(_) => {
                PyViewColumn::from_batch_column_with_type(column, component_type)
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
