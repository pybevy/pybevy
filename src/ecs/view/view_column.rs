//! Opaque column view for zero-copy access via Numba JIT and JAX interop.
//!
//! This module implements the v4.0 "Opaque Handle" architecture, where Python
//! users receive opaque ViewColumn handles that refuse numpy conversion and can
//! be accessed through Numba JIT compilation or JAX array interop.
//!
//! Safety model: ECS-backed columns retain a neutral `BatchColumn` capability.
//! Rust-side accesses check its thread-affine run validity and hold the shared
//! View operation fence. Numba checks validity while unboxing; the native call
//! must finish before its originating system invocation returns.

use std::{
    mem,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy::transform::components::Transform;
use pybevy_bytecodevm::{
    bytecode::{
        python_clip, python_maximum, python_minimum, python_remainder, python_round, python_sign,
        read_field_value, write_field_value,
    },
    view_runtime::{BatchColumn, ViewOperationGuard},
};
use pybevy_core::FieldType;
use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyList},
};

use crate::ecs::{component_type::PyComponentType, view::view::get_component_field_info};

/// Opaque column view that can only be accessed through Numba JIT.
///
/// This struct intentionally does NOT expose `__array_interface__` or allow
/// numpy conversion. The ONLY way to access the data is through Numba JIT
/// functions, where safety checks occur at the call boundary.
///
/// # Safety Model
///
/// - The run-scoped core validity is checked in the Numba unbox() function
/// - Stale or cross-thread handles raise a RuntimeError before pointer exposure
/// - Users cannot get a raw numpy array that bypasses checks
///
/// # Example
///
/// ```python
/// @numba.jit(nopython=True)
/// def kernel(view: ViewColumn):
///     for i in range(len(view)):
///         view[i] = view[i] + 1.0
///
/// def system(view: View[Mut[Transform]]):
///     for batch in view.batch_iter():
///         y = batch.col(Transform).translation.y
///         kernel(y)  # Safety check at call boundary
/// ```
#[pyclass(name = "ViewColumn")]
pub struct PyViewColumn {
    /// Raw pointer to the data (NOT exposed directly to Python).
    ptr: *mut u8,

    /// Number of elements.
    len: usize,

    /// Stride between elements in bytes.
    stride: usize,

    /// Number of bytes reachable from `ptr` within each strided element.
    ///
    /// This may be smaller than `stride` for sub-columns such as Vec3 lanes or
    /// struct slices. Bounds checks must use this value, not `stride`, because
    /// `stride` describes the parent component's row distance.
    element_extent: usize,

    /// Whether writes are allowed by the originating View declaration.
    writable: bool,

    /// Field type (`None` for opaque whole-component views with no single representable type, e.g. Transform or Quat).
    field_type: Option<FieldType>,

    /// Validity token shared across all views from the same batch.
    validity_token: Arc<AtomicBool>,

    /// Core capability that owns ECS-backed storage and its run-scoped fences.
    /// Test and standalone temporary columns use `None` and rely on the token.
    owner: Option<Arc<BatchColumn>>,

    /// Component type for dynamic field resolution (None for primitive columns)
    component_type: Option<*const pyo3::ffi::PyTypeObject>,

    /// Built-in component type for trait-based field access (None for custom/primitive columns)
    builtin_component_type: Option<PyComponentType>,

    /// Owned buffer for temporary arithmetic results (None = ECS-backed pointer)
    owned_data: Option<Vec<u8>>,
}

impl PyViewColumn {
    fn check_live(&self) -> PyResult<()> {
        if !self.validity_token.load(Ordering::Acquire) {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }
        if let Some(owner) = &self.owner {
            owner.check_valid().map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Accessing stale or cross-thread ViewColumn: {error}"
                ))
            })?;
        }
        Ok(())
    }

    fn check_writable(&self) -> PyResult<()> {
        if self.writable {
            Ok(())
        } else {
            Err(PyRuntimeError::new_err(
                "Cannot write through a read-only ViewColumn; use batch.column_mut() and View[Mut[T]]",
            ))
        }
    }

    /// Acquire the core operation fence for one Rust-side pointer operation.
    /// Numba is the deliberate exception: its unbox path checks validity at the
    /// call boundary, then native code uses the retained capability directly.
    fn enter_operation(&self) -> PyResult<Option<ViewOperationGuard>> {
        self.check_live()?;
        self.owner
            .as_ref()
            .map(|owner| {
                owner
                    .enter_operation()
                    .map_err(|error| PyRuntimeError::new_err(error.to_string()))
            })
            .transpose()
    }

    fn enter_pair_operation(
        &self,
        other: &Self,
    ) -> PyResult<(Option<ViewOperationGuard>, Option<ViewOperationGuard>)> {
        let first = self.enter_operation()?;
        let same_runtime = match (&self.owner, &other.owner) {
            (Some(left), Some(right)) => Arc::ptr_eq(left.runtime(), right.runtime()),
            _ => false,
        };
        let second = if same_runtime {
            other.check_live()?;
            None
        } else {
            other.enter_operation()?
        };
        Ok((first, second))
    }

    /// Create an ECS-backed custom component column from a neutral core capability.
    pub(crate) fn from_batch_column_with_type(
        column: BatchColumn,
        component_type: *const pyo3::ffi::PyTypeObject,
    ) -> Self {
        let column = Arc::new(column);
        // SAFETY: this adapter retains `column` for the complete pointer lifetime
        // and all Rust-side dereferences acquire its operation fence.
        let ptr = unsafe { column.raw_ptr_unchecked() };
        Self {
            ptr,
            len: column.len(),
            stride: column.stride(),
            element_extent: column.element_extent(),
            writable: column.is_writable(),
            field_type: None,
            validity_token: Arc::new(AtomicBool::new(true)),
            owner: Some(column),
            component_type: Some(component_type),
            builtin_component_type: None,
            owned_data: None,
        }
    }

    /// Create an ECS-backed built-in component column from a neutral capability.
    pub(crate) fn from_batch_column_with_builtin_type(
        column: BatchColumn,
        builtin_component_type: PyComponentType,
    ) -> Self {
        let column = Arc::new(column);
        // SAFETY: see `from_batch_column_with_type`.
        let ptr = unsafe { column.raw_ptr_unchecked() };
        Self {
            ptr,
            len: column.len(),
            stride: column.stride(),
            element_extent: column.element_extent(),
            writable: column.is_writable(),
            field_type: None,
            validity_token: Arc::new(AtomicBool::new(true)),
            owner: Some(column),
            component_type: None,
            builtin_component_type: Some(builtin_component_type),
            owned_data: None,
        }
    }

    /// Read element at `index` as f64, respecting stride and field type.
    ///
    /// # Panics
    /// Panics if `field_type` is None or a composite variant. Callers must ensure
    /// `check_numeric()` has been called first.
    fn read_f64_at(&self, index: usize) -> f64 {
        let ft = self
            .field_type
            .expect("read_f64_at called on composite/struct column");
        // Safety: `index < len` is enforced by all callers. `ptr` is valid for the
        // system lifetime, guaranteed by the validity token checked in `check_numeric`.
        let ptr = unsafe { self.ptr.add(index * self.stride) };
        // Safety: ptr is aligned and valid for `ft`; `ft` is a scalar variant (not Vec2/Vec3/Vec4).
        unsafe { read_field_value(ptr, ft) }
    }

    /// Write f64 value at `index`, respecting stride and field type.
    ///
    /// # Panics
    /// Panics if `field_type` is None or a composite variant. Callers must ensure
    /// `check_numeric()` has been called first.
    fn write_f64_at(&self, index: usize, value: f64) {
        let ft = self
            .field_type
            .expect("write_f64_at called on composite/struct column");
        // Safety: same as `read_f64_at`.
        let ptr = unsafe { self.ptr.add(index * self.stride) };
        // Safety: ptr is aligned, valid, and not aliased for the duration of this write.
        unsafe { write_field_value(ptr, value, ft) }
    }

    /// Check validity and that this is a scalar field type. Returns the `FieldType` on success.
    fn check_numeric(&self) -> PyResult<FieldType> {
        self.check_live()?;
        match self.field_type {
            Some(
                ft @ (FieldType::F32
                | FieldType::F64
                | FieldType::I32
                | FieldType::I64
                | FieldType::U8
                | FieldType::U32
                | FieldType::U64
                | FieldType::Bool),
            ) => Ok(ft),
            Some(FieldType::Vec2) | Some(FieldType::Vec3) | Some(FieldType::Vec4) | None => {
                Err(PyRuntimeError::new_err(format!(
                    "Arithmetic not supported for dtype '{}'",
                    self.dtype()
                )))
            }
        }
    }

    /// Create an owned ViewColumn from an f64 iterator.
    fn from_f64_iter(
        iter: impl Iterator<Item = f64>,
        len: usize,
        field_type: FieldType,
        validity_token: &Arc<AtomicBool>,
        owner: &Option<Arc<BatchColumn>>,
    ) -> PyResult<Self> {
        let elem_size = field_type.size_bytes();
        let mut buf = vec![0u8; len * elem_size];
        for (i, val) in iter.enumerate() {
            // Safety: `i * elem_size` is within `buf` (allocated as `len * elem_size` bytes).
            let ptr = unsafe { buf.as_mut_ptr().add(i * elem_size) };
            // Safety: ptr is aligned for `field_type`; buf is exclusively owned here.
            unsafe { write_field_value(ptr, val, field_type) };
        }
        Ok(Self {
            ptr: buf.as_mut_ptr(),
            len,
            stride: elem_size,
            element_extent: elem_size,
            writable: true,
            field_type: Some(field_type),
            validity_token: validity_token.clone(),
            owner: owner.clone(),
            component_type: None,
            builtin_component_type: None,
            owned_data: Some(buf),
        })
    }

    /// Apply a unary f64→f64 function element-wise, returning an owned ViewColumn.
    fn unary_op(&self, f: impl Fn(f64) -> f64) -> PyResult<Self> {
        let _operation = self.enter_operation()?;
        let ft = self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(self.read_f64_at(i))),
            self.len,
            ft,
            &self.validity_token,
            &self.owner,
        )
    }

    /// Apply a binary (col, col) → col function element-wise.
    fn binary_op_col(&self, other: &Self, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
        let _operations = self.enter_pair_operation(other)?;
        let ft = self.check_numeric()?;
        other.check_numeric()?;
        if self.len != other.len {
            return Err(PyRuntimeError::new_err(format!(
                "ViewColumn length mismatch: {} vs {}",
                self.len, other.len
            )));
        }
        Self::from_f64_iter(
            (0..self.len).map(|i| f(self.read_f64_at(i), other.read_f64_at(i))),
            self.len,
            ft,
            &self.validity_token,
            &self.owner,
        )
    }

    /// Apply a binary (col, scalar) → col function element-wise.
    fn binary_op_scalar(&self, scalar: f64, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
        let _operation = self.enter_operation()?;
        let ft = self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(self.read_f64_at(i), scalar)),
            self.len,
            ft,
            &self.validity_token,
            &self.owner,
        )
    }

    /// Apply a binary (scalar, col) → col function element-wise.
    fn binary_op_scalar_left(&self, scalar: f64, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
        let _operation = self.enter_operation()?;
        let ft = self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(scalar, self.read_f64_at(i))),
            self.len,
            ft,
            &self.validity_token,
            &self.owner,
        )
    }

    /// Create a sub-column view at a byte offset with a known `FieldType`.
    ///
    /// Internal API — Rust callers should prefer this over `at_offset` to avoid string parsing.
    pub(crate) fn at_offset_typed(
        &self,
        offset: usize,
        field_type: Option<FieldType>,
    ) -> PyResult<Self> {
        let _operation = self.enter_operation()?;
        if self.owned_data.is_some() {
            return Err(PyRuntimeError::new_err(
                "Cannot access sub-columns on a temporary ViewColumn from arithmetic.\n\
                 Assign it back to an ECS-backed column first.",
            ));
        }
        if let Some(owner) = &self.owner {
            let child = Arc::new(
                owner
                    .subcolumn(offset, field_type)
                    .map_err(|error| PyRuntimeError::new_err(error.to_string()))?,
            );
            // SAFETY: the shared runtime validated this child span and the
            // adapter retains its capability for the complete pointer lifetime.
            let ptr = unsafe { child.raw_ptr_unchecked() };
            return Ok(Self {
                ptr,
                len: child.len(),
                stride: child.stride(),
                element_extent: child.element_extent(),
                writable: child.is_writable(),
                field_type,
                validity_token: self.validity_token.clone(),
                owner: Some(child),
                component_type: None,
                builtin_component_type: None,
                owned_data: None,
            });
        }

        // Non-ECS columns retain adapter-local bounds.
        let parent_extent = self.element_extent;
        if offset >= parent_extent {
            return Err(PyRuntimeError::new_err(format!(
                "Offset {offset} out of bounds for '{}' ({} bytes)",
                self.dtype(),
                parent_extent,
            )));
        }
        let child_extent = match field_type {
            Some(ft) => {
                let size = ft.size_bytes();
                let end = offset.checked_add(size).ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Offset {offset} with dtype '{}' overflows bounds for '{}' ({} bytes)",
                        ft.to_numpy_dtype_str(),
                        self.dtype(),
                        parent_extent,
                    ))
                })?;
                if end > parent_extent {
                    return Err(PyRuntimeError::new_err(format!(
                        "Offset {offset} with dtype '{}' out of bounds for '{}' ({} bytes)",
                        ft.to_numpy_dtype_str(),
                        self.dtype(),
                        parent_extent,
                    )));
                }
                size
            }
            None => parent_extent - offset,
        };
        Ok(Self {
            // Safety: `offset < parent_extent`, and parent_extent was derived
            // from the same strided ECS element or owned buffer. The resulting
            // pointer is still within the same allocation; validity is inherited
            // via the shared token.
            ptr: unsafe { self.ptr.add(offset) },
            len: self.len,
            stride: self.stride,
            element_extent: child_extent,
            writable: self.writable,
            field_type,
            validity_token: self.validity_token.clone(),
            owner: self.owner.clone(),
            component_type: None,
            builtin_component_type: None,
            owned_data: None,
        })
    }
}

// SAFETY: ECS-backed columns retain `BatchColumn`, whose thread-affine validity
// check and operation fence guard every Rust-side dereference. The Numba escape
// hatch checks validity while unboxing and must finish native pointer use before
// the system returns; Bevy scheduler access and exact batch ranges exclude ECS
// aliases during that window. Test-owned buffers are never exposed across tests.
unsafe impl Send for PyViewColumn {}
// SAFETY: sharing the wrapper does not bypass those checks. Cross-thread Python
// access fails the core validity check before pointer exposure or dereference.
unsafe impl Sync for PyViewColumn {}

#[pymethods]
impl PyViewColumn {
    /// Block __array__ attribute access to prevent numpy conversion.
    #[getter(__array__)]
    fn get_array(&self) -> PyResult<()> {
        Err(PyRuntimeError::new_err(
            "ViewColumn cannot be converted to numpy array.\n\
             This is an opaque handle that can only be used with @numba.jit functions.\n\
             \n\
             Example:\n\
             @numba.jit(nopython=True)\n\
             def kernel(view: ViewColumn):\n\
                 for i in range(len(view)):\n\
                     view[i] = view[i] + 1.0\n\
             \n\
             kernel(y_pos)  # This works!",
        ))
    }

    /// Block __array_interface__ attribute access.
    #[getter(__array_interface__)]
    fn get_array_interface(&self) -> PyResult<()> {
        self.get_array()
    }

    /// Check if this view is still valid.
    ///
    /// Returns False if the system that created this view has finished execution.
    #[getter]
    fn is_valid(&self) -> bool {
        self.check_live().is_ok()
    }

    /// Get the raw pointer (for Numba unbox only).
    ///
    /// This checks validity before returning the pointer.
    #[getter]
    fn ptr(&self) -> PyResult<usize> {
        if self.check_live().is_err() {
            return Err(PyRuntimeError::new_err(
                "CRITICAL: Accessing stale ViewColumn!\n\
                 This view is only valid within the system that created it.\n\
                 Do not store this object in global variables.",
            ));
        }
        Ok(self.ptr as usize)
    }

    /// Get the number of elements.
    #[getter]
    fn len(&self) -> usize {
        self.len
    }

    /// Support Python's len() function.
    fn __len__(&self) -> usize {
        self.len
    }

    /// Get the stride in bytes.
    #[getter]
    fn stride(&self) -> usize {
        self.stride
    }

    /// Whether this column permits writes.
    #[getter]
    fn writable(&self) -> bool {
        self.writable
    }

    /// Get the NumPy dtype string (e.g., "f4", "i8", "u1", "struct").
    #[getter]
    fn dtype(&self) -> &'static str {
        match self.field_type {
            Some(ft) => ft.to_numpy_dtype_str(),
            None => "struct",
        }
    }

    /// Create a sub-column view at a byte offset (for field peeling).
    ///
    /// This is the Python-facing API; Rust internals should use `at_offset_typed`.
    ///
    /// # Arguments
    ///
    /// - `offset`: Byte offset from the current pointer
    /// - `dtype`: NumPy dtype string (e.g., "f4", "i8", "u1") or "struct" for composite
    pub fn at_offset(&self, offset: usize, dtype: &str) -> PyResult<Self> {
        let field_type = match dtype {
            "u1" => Some(FieldType::Bool),
            "f4" => Some(FieldType::F32),
            "f8" => Some(FieldType::F64),
            "i4" => Some(FieldType::I32),
            "i8" => Some(FieldType::I64),
            "u4" => Some(FieldType::U32),
            "u8" => Some(FieldType::U64),
            "vec2" => Some(FieldType::Vec2),
            "vec3" => Some(FieldType::Vec3),
            "vec4" => Some(FieldType::Vec4),
            "struct" => None,
            _ => {
                return Err(PyRuntimeError::new_err(format!(
                    "Unknown dtype '{}'. Use one of: f4, f8, i4, i8, u4, u8, u1, vec2, vec3, vec4, struct",
                    dtype
                )));
            }
        };
        self.at_offset_typed(offset, field_type)
    }

    /// Helper method for debugging: peek at a single value (with safety check).
    pub fn peek(&self, index: usize) -> PyResult<f64> {
        let _operation = self.enter_operation()?;
        self.check_numeric()?;
        if index >= self.len {
            return Err(PyRuntimeError::new_err(format!(
                "Index {} out of bounds (len = {})",
                index, self.len
            )));
        }
        Ok(self.read_f64_at(index))
    }

    /// Helper method for debugging: convert to Python list (with copy).
    pub fn to_list(&self, py: Python) -> PyResult<Py<PyAny>> {
        let _operation = self.enter_operation()?;
        self.check_numeric()?;
        let values: Vec<f64> = (0..self.len).map(|i| self.read_f64_at(i)).collect();
        Ok(PyList::new(py, values)?.into_any().unbind())
    }

    /// Handle attribute access for structured field access (e.g., .translation, .x).
    fn __getattr__(&self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        // Priority 1: Built-in component with trait-based field access
        // Skip Transform here - it has composite fields (Vec3/Quat) that need special wrapper handling
        if let Some(ref comp_type) = self.builtin_component_type {
            // Transform has composite fields (Vec3/Quat), handle in hardcoded fallback
            let is_composite = match comp_type {
                PyComponentType::Dynamic(type_ptr) => {
                    pybevy_core::registry::global_registry::get_bridge_by_py_type(*type_ptr)
                        .map(|b| b.name() == "Transform")
                        .unwrap_or(false)
                }
                _ => false,
            };
            if !is_composite
                && let Ok((offset, field_type)) = get_component_field_info(comp_type, name)
            {
                match field_type {
                    FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => {
                        // Composite fields for built-in components shouldn't reach here
                        // (they use bridge which returns individual scalar sub-fields)
                        return Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                            "Cannot access composite field '{}' as a raw column. \
                                 Use .{}.x, .{}.y, .{}.z for individual component access.",
                            name, name, name, name,
                        )));
                    }
                    _ => {
                        let field_col = self.at_offset_typed(offset, Some(field_type))?;
                        return Ok(Py::new(py, field_col)?.into());
                    }
                }
            }
        }

        // Priority 2: Custom component with dynamic field access
        if let Some(type_ptr) = self.component_type {
            use crate::ecs::component_layout::{
                ComponentLayout, ComponentLayoutExt, PrimitiveType, PrimitiveTypeExt,
            };

            // Safety: `type_ptr` was captured from a live Python type object and the GIL
            // is held here, so the pointer is valid for the duration of this call.
            let py_type =
                unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };

            if let Ok(cls) = py_type.cast::<pyo3::types::PyType>()
                && let Ok(layout) = ComponentLayout::from_annotations(cls)
            {
                // Find field in layout
                for field in &layout.fields {
                    if field.name == name {
                        match field.field_type {
                            PrimitiveType::Vec3 => {
                                let vec3_col =
                                    self.at_offset_typed(field.offset, Some(FieldType::Vec3))?;
                                let viewcolumn_accessors =
                                    py.import("pybevy.ecs.view_accessors")?;
                                let vec3_wrapper =
                                    viewcolumn_accessors.getattr("Vec3ViewColumn")?;
                                return Ok(vec3_wrapper.call1((vec3_col,))?.into());
                            }
                            PrimitiveType::Vec2 => {
                                let vec2_col =
                                    self.at_offset_typed(field.offset, Some(FieldType::Vec2))?;
                                let viewcolumn_accessors =
                                    py.import("pybevy.ecs.view_accessors")?;
                                let vec2_wrapper =
                                    viewcolumn_accessors.getattr("Vec2ViewColumn")?;
                                return Ok(vec2_wrapper.call1((vec2_col,))?.into());
                            }
                            _ => {
                                let field_col = self.at_offset_typed(
                                    field.offset,
                                    Some(field.field_type.to_field_type()),
                                )?;
                                return Ok(Py::new(py, field_col)?.into());
                            }
                        }
                    }
                }

                // Field not found in layout
                let available: Vec<&str> = layout.fields.iter().map(|f| f.name.as_str()).collect();
                return Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                    "Component has no field '{}' (available: {})",
                    name,
                    available.join(", ")
                )));
            }
        }

        // Priority 3: Fallback to hardcoded fields (for backwards compatibility and special cases)
        let viewcolumn_accessors = py.import("pybevy.ecs.view_accessors")?;

        match name {
            // Transform fields — offsets derived from the actual type, validated by layout_assertions.rs
            "rotation" => {
                let quat_col = self
                    .at_offset_typed(mem::offset_of!(Transform, rotation), Some(FieldType::Vec4))?;
                let quat_wrapper = viewcolumn_accessors.getattr("QuatViewColumn")?;
                Ok(quat_wrapper.call1((quat_col,))?.into())
            }
            "translation" => {
                let vec3_col = self.at_offset_typed(
                    mem::offset_of!(Transform, translation),
                    Some(FieldType::Vec3),
                )?;
                let vec3_wrapper = viewcolumn_accessors.getattr("Vec3ViewColumn")?;
                Ok(vec3_wrapper.call1((vec3_col,))?.into())
            }
            "scale" => {
                let vec3_col =
                    self.at_offset_typed(mem::offset_of!(Transform, scale), Some(FieldType::Vec3))?;
                let vec3_wrapper = viewcolumn_accessors.getattr("Vec3ViewColumn")?;
                Ok(vec3_wrapper.call1((vec3_col,))?.into())
            }
            // Vec2/Vec3/Vec4 scalar sub-fields
            "x" => Ok(Py::new(py, self.at_offset_typed(0, Some(FieldType::F32))?)?.into()),
            "y" => Ok(Py::new(py, self.at_offset_typed(4, Some(FieldType::F32))?)?.into()),
            "z" => Ok(Py::new(py, self.at_offset_typed(8, Some(FieldType::F32))?)?.into()),
            "w" => Ok(Py::new(py, self.at_offset_typed(12, Some(FieldType::F32))?)?.into()),
            _ => Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                "ViewColumn has no attribute '{}'",
                name
            ))),
        }
    }

    fn __repr__(&self) -> String {
        if self.is_valid() {
            format!(
                "ViewColumn(len={}, stride={}, dtype='{}', valid=True)",
                self.len,
                self.stride,
                self.dtype()
            )
        } else {
            format!(
                "ViewColumn(len={}, stride={}, dtype='{}', valid=False [STALE])",
                self.len,
                self.stride,
                self.dtype()
            )
        }
    }

    /// Support indexing for Numba JIT compatibility.
    fn __getitem__(&self, index: isize) -> PyResult<f64> {
        let _operation = self.enter_operation()?;
        self.check_numeric()?;

        let idx = if index < 0 {
            let neg_idx = (-index) as usize;
            if neg_idx > self.len {
                return Err(pyo3::exceptions::PyIndexError::new_err(format!(
                    "Index {} out of bounds (len = {})",
                    index, self.len
                )));
            }
            self.len - neg_idx
        } else {
            index as usize
        };

        if idx >= self.len {
            return Err(pyo3::exceptions::PyIndexError::new_err(format!(
                "Index {} out of bounds (len = {})",
                index, self.len
            )));
        }

        Ok(self.read_f64_at(idx))
    }

    /// Support item assignment for Numba JIT compatibility.
    fn __setitem__(&mut self, index: isize, value: f64) -> PyResult<()> {
        let _operation = self.enter_operation()?;
        self.check_writable()?;
        self.check_numeric()?;

        let idx = if index < 0 {
            let neg_idx = (-index) as usize;
            if neg_idx > self.len {
                return Err(pyo3::exceptions::PyIndexError::new_err(format!(
                    "Index {} out of bounds (len = {})",
                    index, self.len
                )));
            }
            self.len - neg_idx
        } else {
            index as usize
        };

        if idx >= self.len {
            return Err(pyo3::exceptions::PyIndexError::new_err(format!(
                "Index {} out of bounds (len = {})",
                index, self.len
            )));
        }

        self.write_f64_at(idx, value);
        Ok(())
    }

    /// Copy column data into a contiguous bytes buffer, preserving native dtype.
    ///
    /// Returns bytes in the column's native dtype (f4/f8/i4/i8), not f64.
    /// The output is tightly packed (no stride gaps), suitable for wrapping
    /// with `numpy.frombuffer()` or JAX array construction.
    pub fn to_contiguous_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let _operation = self.enter_operation()?;
        let elem_size = self
            .field_type
            .ok_or_else(|| {
                PyRuntimeError::new_err("Cannot get bytes from a composite/struct column")
            })?
            .size_bytes();
        let mut buf = vec![0u8; self.len * elem_size];
        for i in 0..self.len {
            let dst_offset = i * elem_size;
            // Safety: source pointer is within a valid ECS column (validity checked above);
            // destination is within the exclusively-owned `buf`. Ranges don't overlap.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.ptr.add(i * self.stride),
                    buf[dst_offset..].as_mut_ptr(),
                    elem_size,
                );
            }
        }
        Ok(PyBytes::new(py, &buf))
    }

    /// Bulk write from a Python bytes/buffer into ECS storage.
    ///
    /// The input must be tightly packed data in the column's native dtype.
    /// Handles stride-aware writes for non-contiguous archetype layouts.
    pub fn write_from_buffer(&self, data: &[u8]) -> PyResult<()> {
        let _operation = self.enter_operation()?;
        self.check_writable()?;
        let elem_size = self
            .field_type
            .ok_or_else(|| PyRuntimeError::new_err("Cannot write to a composite/struct column"))?
            .size_bytes();
        let expected_len = self.len * elem_size;
        if data.len() != expected_len {
            return Err(PyRuntimeError::new_err(format!(
                "Buffer size mismatch: expected {} bytes ({} elements × {} bytes), got {}",
                expected_len,
                self.len,
                elem_size,
                data.len()
            )));
        }
        for i in 0..self.len {
            let src_offset = i * elem_size;
            // Safety: source is a validated slice of the correct length; destination is a
            // valid ECS column pointer (validity checked above). Ranges don't overlap.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data[src_offset..].as_ptr(),
                    self.ptr.add(i * self.stride),
                    elem_size,
                );
            }
        }
        Ok(())
    }

    fn __mul__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), |a, b| a * b)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, |a, b| a * b)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __rmul__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.__mul__(py, other)
    }

    fn __add__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), |a, b| a + b)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, |a, b| a + b)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __radd__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        self.__add__(py, other)
    }

    fn __sub__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), |a, b| a - b)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, |a, b| a - b)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __rsub__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar_left(scalar, |a, b| a - b)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __truediv__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), |a, b| a / b)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, |a, b| a / b)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __rtruediv__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar_left(scalar, |a, b| a / b)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __pow__(
        &self,
        py: Python,
        other: &Bound<PyAny>,
        _modulo: Option<&Bound<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), |a, b| a.powf(b))?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, |a, b| a.powf(b))?;
        Ok(Py::new(py, result)?.into())
    }

    fn __rpow__(
        &self,
        py: Python,
        other: &Bound<PyAny>,
        _modulo: Option<&Bound<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar_left(scalar, |a, b| a.powf(b))?;
        Ok(Py::new(py, result)?.into())
    }

    fn __mod__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), python_remainder)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, python_remainder)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __rmod__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar_left(scalar, python_remainder)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __neg__(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(|a| -a)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __abs__(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(|a| a.abs())?;
        Ok(Py::new(py, result)?.into())
    }

    fn sin(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::sin)?;
        Ok(Py::new(py, result)?.into())
    }

    fn cos(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::cos)?;
        Ok(Py::new(py, result)?.into())
    }

    fn tan(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::tan)?;
        Ok(Py::new(py, result)?.into())
    }

    fn asin(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::asin)?;
        Ok(Py::new(py, result)?.into())
    }

    fn acos(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::acos)?;
        Ok(Py::new(py, result)?.into())
    }

    fn atan(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::atan)?;
        Ok(Py::new(py, result)?.into())
    }

    fn sqrt(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::sqrt)?;
        Ok(Py::new(py, result)?.into())
    }

    #[pyo3(name = "abs")]
    fn abs_method(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::abs)?;
        Ok(Py::new(py, result)?.into())
    }

    fn floor(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::floor)?;
        Ok(Py::new(py, result)?.into())
    }

    fn ceil(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::ceil)?;
        Ok(Py::new(py, result)?.into())
    }

    fn round(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(python_round)?;
        Ok(Py::new(py, result)?.into())
    }

    fn exp(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::exp)?;
        Ok(Py::new(py, result)?.into())
    }

    fn ln(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::ln)?;
        Ok(Py::new(py, result)?.into())
    }

    fn log10(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::log10)?;
        Ok(Py::new(py, result)?.into())
    }

    fn log2(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::log2)?;
        Ok(Py::new(py, result)?.into())
    }

    fn sign(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(python_sign)?;
        Ok(Py::new(py, result)?.into())
    }

    fn fract(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::fract)?;
        Ok(Py::new(py, result)?.into())
    }

    fn min(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), python_minimum)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, python_minimum)?;
        Ok(Py::new(py, result)?.into())
    }

    fn max(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), python_maximum)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, python_maximum)?;
        Ok(Py::new(py, result)?.into())
    }

    fn clamp(&self, py: Python, min_val: f64, max_val: f64) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(|a| python_clip(a, min_val, max_val))?;
        Ok(Py::new(py, result)?.into())
    }

    fn lerp(&self, py: Python, other: &Bound<PyAny>, t: f64) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), |a, b| a + (b - a) * t)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, |a, b| a + (b - a) * t)?;
        Ok(Py::new(py, result)?.into())
    }

    /// Assign values from another ViewColumn or a scalar into this column.
    ///
    /// Used by Python `__setattr__` on wrapper classes to enable:
    ///     batch.column_mut(Transform).translation.y = (col * 0.5).sin()
    pub fn set(&self, value: &Bound<PyAny>) -> PyResult<()> {
        self.check_writable()?;
        if let Ok(col) = value.cast::<PyViewColumn>() {
            let src = col.borrow();
            let _operations = self.enter_pair_operation(&src)?;
            self.check_numeric()?;
            src.check_numeric()?;
            if self.len != src.len {
                return Err(PyRuntimeError::new_err(format!(
                    "ViewColumn length mismatch: {} vs {}",
                    self.len, src.len
                )));
            }
            for i in 0..self.len {
                self.write_f64_at(i, src.read_f64_at(i));
            }
            return Ok(());
        }

        // Scalar broadcast
        let _operation = self.enter_operation()?;
        self.check_numeric()?;
        let scalar: f64 = value.extract().map_err(|_| {
            PyRuntimeError::new_err("Cannot assign: value must be a ViewColumn or a number")
        })?;
        for i in 0..self.len {
            self.write_f64_at(i, scalar);
        }
        Ok(())
    }
}
