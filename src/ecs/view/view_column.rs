//! Opaque column view for zero-copy access via Numba JIT and JAX interop.
//!
//! This module implements the v4.0 "Opaque Handle" architecture, where Python
//! users receive opaque ViewColumn handles that refuse numpy conversion and can
//! be accessed through Numba JIT compilation or JAX array interop.
//!
//! Safety model: Arc<AtomicBool> validity token is checked at the Numba call
//! boundary (in the unbox() function) and in bulk read/write methods,
//! preventing use-after-free bugs.

use std::{
    mem,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy::transform::components::Transform;
use pybevy_bytecodevm::bytecode::{read_field_value, write_field_value};
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
/// - The validity token is checked in the Numba unbox() function
/// - If the token is poisoned, unbox() raises a RuntimeError
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

    /// Field type (`None` for opaque whole-component views with no single representable type, e.g. Transform or Quat).
    field_type: Option<FieldType>,

    /// Validity token shared across all views from the same batch.
    validity_token: Arc<AtomicBool>,

    /// Component type for dynamic field resolution (None for primitive columns)
    component_type: Option<*const pyo3::ffi::PyTypeObject>,

    /// Built-in component type for trait-based field access (None for custom/primitive columns)
    builtin_component_type: Option<PyComponentType>,

    /// Owned buffer for temporary arithmetic results (None = ECS-backed pointer)
    owned_data: Option<Vec<u8>>,
}

impl PyViewColumn {
    /// Create a ViewColumn with component type info for dynamic field access.
    /// Whole-component columns are always opaque structs (field_type = None).
    ///
    /// # Safety
    /// `ptr` must point to the first element of a valid ECS column with `len` elements
    /// spaced `stride` bytes apart. The pointer must remain valid for the lifetime of
    /// the validity token (i.e. until the system finishes execution).
    pub(crate) unsafe fn from_raw_parts_with_type(
        ptr: *const u8,
        len: usize,
        stride: usize,
        validity_token: Arc<AtomicBool>,
        component_type: *const pyo3::ffi::PyTypeObject,
    ) -> Self {
        Self {
            ptr: ptr as *mut u8,
            len,
            stride,
            field_type: None,
            validity_token,
            component_type: Some(component_type),
            builtin_component_type: None,
            owned_data: None,
        }
    }

    /// Create a ViewColumn with built-in component type for trait-based field access.
    /// Whole-component columns are always opaque structs (field_type = None).
    ///
    /// # Safety
    /// Same requirements as `from_raw_parts_with_type`.
    pub(crate) unsafe fn from_raw_parts_with_builtin_type(
        ptr: *const u8,
        len: usize,
        stride: usize,
        validity_token: Arc<AtomicBool>,
        builtin_component_type: PyComponentType,
    ) -> Self {
        Self {
            ptr: ptr as *mut u8,
            len,
            stride,
            field_type: None,
            validity_token,
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
        if !self.validity_token.load(Ordering::Relaxed) {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }
        match self.field_type {
            Some(
                ft @ (FieldType::F32
                | FieldType::F64
                | FieldType::I32
                | FieldType::I64
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
            field_type: Some(field_type),
            validity_token: validity_token.clone(),
            component_type: None,
            builtin_component_type: None,
            owned_data: Some(buf),
        })
    }

    /// Apply a unary f64→f64 function element-wise, returning an owned ViewColumn.
    fn unary_op(&self, f: impl Fn(f64) -> f64) -> PyResult<Self> {
        let ft = self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(self.read_f64_at(i))),
            self.len,
            ft,
            &self.validity_token,
        )
    }

    /// Apply a binary (col, col) → col function element-wise.
    fn binary_op_col(&self, other: &Self, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
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
        )
    }

    /// Apply a binary (col, scalar) → col function element-wise.
    fn binary_op_scalar(&self, scalar: f64, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
        let ft = self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(self.read_f64_at(i), scalar)),
            self.len,
            ft,
            &self.validity_token,
        )
    }

    /// Apply a binary (scalar, col) → col function element-wise.
    fn binary_op_scalar_left(&self, scalar: f64, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
        let ft = self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(scalar, self.read_f64_at(i))),
            self.len,
            ft,
            &self.validity_token,
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
        if self.owned_data.is_some() {
            return Err(PyRuntimeError::new_err(
                "Cannot access sub-columns on a temporary ViewColumn from arithmetic.\n\
                 Assign it back to an ECS-backed column first.",
            ));
        }
        // Validate against the element extent: for typed columns use the type's byte size,
        // for opaque struct columns fall back to the stride.
        let extent = match self.field_type {
            Some(ft) => ft.size_bytes(),
            None => self.stride,
        };
        if extent > 0 && offset >= extent {
            return Err(PyRuntimeError::new_err(format!(
                "Offset {offset} out of bounds for '{}' ({} bytes)",
                self.dtype(),
                extent,
            )));
        }
        Ok(Self {
            // Safety: `offset < extent <= stride`, so the resulting pointer is still within
            // the same ECS column allocation. Validity is inherited via the shared token.
            ptr: unsafe { self.ptr.add(offset) },
            len: self.len,
            stride: self.stride,
            field_type,
            validity_token: self.validity_token.clone(),
            component_type: None,
            builtin_component_type: None,
            owned_data: None,
        })
    }
}

// Safety: `ptr` is only accessed while the validity token is live (checked before every
// read/write), and `Arc<AtomicBool>` coordinates access across threads. The raw pointer
// is never aliased mutably while any shared reference exists.
unsafe impl Send for PyViewColumn {}
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
        self.validity_token.load(Ordering::Relaxed)
    }

    /// Get the raw pointer (for Numba unbox only).
    ///
    /// This checks validity before returning the pointer.
    #[getter]
    fn ptr(&self) -> PyResult<usize> {
        if !self.is_valid() {
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
            "struct" => None,
            _ => {
                return Err(PyRuntimeError::new_err(format!(
                    "Unknown dtype '{}'. Use one of: f4, f8, i4, i8, u4, u8, u1, struct",
                    dtype
                )));
            }
        };
        self.at_offset_typed(offset, field_type)
    }

    /// Helper method for debugging: peek at a single value (with safety check).
    pub fn peek(&self, index: usize) -> PyResult<f64> {
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
        if !self.is_valid() {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }
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
        if !self.validity_token.load(Ordering::Relaxed) {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }
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
            let result = self.binary_op_col(&col.borrow(), |a, b| a % b)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, |a, b| a % b)?;
        Ok(Py::new(py, result)?.into())
    }

    fn __rmod__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar_left(scalar, |a, b| a % b)?;
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
        let result = self.unary_op(f64::round)?;
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
        let result = self.unary_op(f64::signum)?;
        Ok(Py::new(py, result)?.into())
    }

    fn fract(&self, py: Python) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(f64::fract)?;
        Ok(Py::new(py, result)?.into())
    }

    fn min(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), f64::min)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, f64::min)?;
        Ok(Py::new(py, result)?.into())
    }

    fn max(&self, py: Python, other: &Bound<PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(col) = other.cast::<PyViewColumn>() {
            let result = self.binary_op_col(&col.borrow(), f64::max)?;
            return Ok(Py::new(py, result)?.into());
        }
        let scalar: f64 = other.extract()?;
        let result = self.binary_op_scalar(scalar, f64::max)?;
        Ok(Py::new(py, result)?.into())
    }

    fn clamp(&self, py: Python, min_val: f64, max_val: f64) -> PyResult<Py<PyAny>> {
        let result = self.unary_op(|a| a.clamp(min_val, max_val))?;
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
        self.check_numeric()?;

        if let Ok(col) = value.cast::<PyViewColumn>() {
            let src = col.borrow();
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
        let scalar: f64 = value.extract().map_err(|_| {
            PyRuntimeError::new_err("Cannot assign: value must be a ViewColumn or a number")
        })?;
        for i in 0..self.len {
            self.write_f64_at(i, scalar);
        }
        Ok(())
    }
}
