//! Opaque column view for zero-copy access via Numba JIT and JAX interop.
//!
//! This module implements the v4.0 "Opaque Handle" architecture, where Python
//! users receive opaque ViewColumn handles that refuse numpy conversion and can
//! be accessed through Numba JIT compilation or JAX array interop.
//!
//! Safety model: Arc<AtomicBool> validity token is checked at the Numba call
//! boundary (in the unbox() function) and in bulk read/write methods,
//! preventing use-after-free bugs.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyList},
};

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
    ptr: usize,

    /// Number of elements.
    len: usize,

    /// Stride between elements in bytes.
    stride: usize,

    /// NumPy dtype string (e.g., "f4" for float32, "i4" for int32).
    dtype: String,

    /// Validity token shared across all views from the same batch.
    validity_token: Arc<AtomicBool>,

    /// Component type for dynamic field resolution (None for primitive columns)
    component_type: Option<*const pyo3::ffi::PyTypeObject>,

    /// Built-in component type for trait-based field access (None for custom/primitive columns)
    builtin_component_type: Option<crate::ecs::component_type::PyComponentType>,

    /// Owned buffer for temporary arithmetic results (None = ECS-backed pointer)
    owned_data: Option<Vec<u8>>,
}

impl PyViewColumn {
    /// Create a ViewColumn with component type info for dynamic field access
    pub(crate) unsafe fn from_raw_parts_with_type(
        ptr: *const u8,
        len: usize,
        stride: usize,
        dtype: String,
        validity_token: Arc<AtomicBool>,
        component_type: *const pyo3::ffi::PyTypeObject,
    ) -> Self {
        Self {
            ptr: ptr as usize,
            len,
            stride,
            dtype,
            validity_token,
            component_type: Some(component_type),
            builtin_component_type: None,
            owned_data: None,
        }
    }

    /// Create a ViewColumn with built-in component type for trait-based field access
    pub(crate) unsafe fn from_raw_parts_with_builtin_type(
        ptr: *const u8,
        len: usize,
        stride: usize,
        dtype: String,
        validity_token: Arc<AtomicBool>,
        builtin_component_type: crate::ecs::component_type::PyComponentType,
    ) -> Self {
        Self {
            ptr: ptr as usize,
            len,
            stride,
            dtype,
            validity_token,
            component_type: None,
            builtin_component_type: Some(builtin_component_type),
            owned_data: None,
        }
    }

    /// Byte size for a given dtype string.
    fn dtype_size(dtype: &str) -> PyResult<usize> {
        match dtype {
            "u1" => Ok(1),
            "f4" | "i4" | "u4" => Ok(4),
            "f8" | "i8" | "u8" => Ok(8),
            _ => Err(PyRuntimeError::new_err(format!(
                "Arithmetic not supported for dtype '{dtype}'"
            ))),
        }
    }

    /// Read element at `index` as f64, respecting stride and dtype.
    fn read_f64_at(&self, index: usize) -> f64 {
        let addr = self.ptr + index * self.stride;
        unsafe {
            match self.dtype.as_str() {
                "u1" => *(addr as *const u8) as f64,
                "f4" => *(addr as *const f32) as f64,
                "f8" => *(addr as *const f64),
                "i4" => *(addr as *const i32) as f64,
                "i8" => *(addr as *const i64) as f64,
                "u4" => *(addr as *const u32) as f64,
                "u8" => *(addr as *const u64) as f64,
                _ => *(addr as *const f32) as f64,
            }
        }
    }

    /// Write f64 value at `index`, respecting stride and dtype.
    fn write_f64_at(&self, index: usize, value: f64) {
        let addr = self.ptr + index * self.stride;
        unsafe {
            match self.dtype.as_str() {
                "u1" => *(addr as *mut u8) = (value != 0.0) as u8,
                "f4" => *(addr as *mut f32) = value as f32,
                "f8" => *(addr as *mut f64) = value,
                "i4" => *(addr as *mut i32) = value as i32,
                "i8" => *(addr as *mut i64) = value as i64,
                "u4" => *(addr as *mut u32) = value as u32,
                "u8" => *(addr as *mut u64) = value as u64,
                _ => *(addr as *mut f32) = value as f32,
            }
        }
    }

    /// Check validity and reject non-numeric dtypes.
    fn check_numeric(&self) -> PyResult<()> {
        if !self.validity_token.load(Ordering::Relaxed) {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }
        match self.dtype.as_str() {
            "f4" | "f8" | "i4" | "i8" | "u4" | "u8" | "u1" => Ok(()),
            _ => Err(PyRuntimeError::new_err(format!(
                "Arithmetic not supported for dtype '{}'",
                self.dtype
            ))),
        }
    }

    /// Create an owned ViewColumn from an f64 iterator.
    fn from_f64_iter(
        iter: impl Iterator<Item = f64>,
        len: usize,
        dtype: &str,
        validity_token: &Arc<AtomicBool>,
    ) -> PyResult<Self> {
        let elem_size = Self::dtype_size(dtype)?;
        let mut buf = vec![0u8; len * elem_size];
        let buf_ptr = buf.as_mut_ptr() as usize;
        // Write using the target dtype
        for (i, val) in iter.enumerate() {
            let addr = buf_ptr + i * elem_size;
            unsafe {
                match dtype {
                    "f4" => *(addr as *mut f32) = val as f32,
                    "f8" => *(addr as *mut f64) = val,
                    "i4" => *(addr as *mut i32) = val as i32,
                    "i8" => *(addr as *mut i64) = val as i64,
                    "u4" => *(addr as *mut u32) = val as u32,
                    "u8" => *(addr as *mut u64) = val as u64,
                    _ => *(addr as *mut f32) = val as f32,
                }
            }
        }
        let ptr = buf.as_ptr() as usize;
        Ok(Self {
            ptr,
            len,
            stride: elem_size,
            dtype: dtype.to_string(),
            validity_token: validity_token.clone(),
            component_type: None,
            builtin_component_type: None,
            owned_data: Some(buf),
        })
    }

    /// Apply a unary f64→f64 function element-wise, returning an owned ViewColumn.
    fn unary_op(&self, f: impl Fn(f64) -> f64) -> PyResult<Self> {
        self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(self.read_f64_at(i))),
            self.len,
            &self.dtype,
            &self.validity_token,
        )
    }

    /// Apply a binary (col, col) → col function element-wise.
    fn binary_op_col(&self, other: &Self, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
        self.check_numeric()?;
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
            &self.dtype,
            &self.validity_token,
        )
    }

    /// Apply a binary (col, scalar) → col function element-wise.
    fn binary_op_scalar(&self, scalar: f64, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
        self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(self.read_f64_at(i), scalar)),
            self.len,
            &self.dtype,
            &self.validity_token,
        )
    }

    /// Apply a binary (scalar, col) → col function element-wise.
    fn binary_op_scalar_left(&self, scalar: f64, f: impl Fn(f64, f64) -> f64) -> PyResult<Self> {
        self.check_numeric()?;
        Self::from_f64_iter(
            (0..self.len).map(|i| f(scalar, self.read_f64_at(i))),
            self.len,
            &self.dtype,
            &self.validity_token,
        )
    }
}

// Implement Sync + Send since we're manually managing the Arc
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
        Ok(self.ptr)
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

    /// Get the NumPy dtype string.
    #[getter]
    fn dtype(&self) -> &str {
        &self.dtype
    }

    /// Create a sub-column view at a byte offset (for field peeling).
    ///
    /// This is used to implement `.translation.y` syntax by creating a new
    /// view with adjusted pointer offset.
    ///
    /// # Arguments
    ///
    /// - `offset`: Byte offset from the current pointer
    /// - `dtype`: NumPy dtype string for the sub-column
    pub fn at_offset(&self, offset: usize, dtype: &str) -> PyResult<Self> {
        if self.owned_data.is_some() {
            return Err(PyRuntimeError::new_err(
                "Cannot access sub-columns on a temporary ViewColumn from arithmetic.\n\
                 Assign it back to an ECS-backed column first.",
            ));
        }
        if self.stride > 0 && offset >= self.stride {
            return Err(PyRuntimeError::new_err(format!(
                "Offset {offset} exceeds stride {} — would read out of bounds",
                self.stride
            )));
        }
        Ok(Self {
            ptr: self.ptr + offset,
            len: self.len,
            stride: self.stride,
            dtype: dtype.to_string(),
            validity_token: self.validity_token.clone(),
            component_type: None,
            builtin_component_type: None,
            owned_data: None,
        })
    }

    /// Helper method for debugging: peek at a single value (with safety check).
    pub fn peek(&self, index: usize) -> PyResult<f64> {
        if !self.is_valid() {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }
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
        if !self.is_valid() {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }
        let values: Vec<f64> = (0..self.len).map(|i| self.read_f64_at(i)).collect();
        Ok(PyList::new(py, values)?.into_any().unbind())
    }

    /// Handle attribute access for structured field access (e.g., .translation, .x).
    fn __getattr__(&self, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        // Priority 1: Built-in component with trait-based field access
        // Skip Transform here - it has composite fields (Vec3/Quat) that need special wrapper handling
        if let Some(ref comp_type) = self.builtin_component_type {
            use pybevy_bytecodevm::bytecode::FieldType;

            use crate::ecs::{
                component_type::PyComponentType, view::view::get_component_field_info,
            };

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
                let dtype = match field_type {
                    FieldType::F32 => "f4",
                    FieldType::F64 => "f8",
                    FieldType::I32 => "i4",
                    FieldType::I64 => "i8",
                    FieldType::U32 => "u4",
                    FieldType::U64 => "u8",
                    FieldType::Bool => "u1",
                    FieldType::Vec3 | FieldType::Vec2 => {
                        // Composite fields for built-in components shouldn't reach here
                        // (they use bridge which returns individual scalar sub-fields)
                        return Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                            "Cannot access composite field '{}' as a raw column. \
                                 Use .{}.x, .{}.y, .{}.z for individual component access.",
                            name, name, name, name,
                        )));
                    }
                };

                let field_col = self.at_offset(offset, dtype)?;
                return Ok(Py::new(py, field_col)?.into());
            }
        }

        // Priority 2: Custom component with dynamic field access
        if let Some(type_ptr) = self.component_type {
            use crate::ecs::component_layout::{ComponentLayout, PrimitiveType};

            let py_type =
                unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };

            if let Ok(cls) = py_type.cast::<pyo3::types::PyType>()
                && let Ok(layout) = ComponentLayout::from_annotations(cls)
            {
                // Find field in layout
                for field in &layout.fields {
                    if field.name == name {
                        // Determine dtype from field type
                        let dtype = match field.field_type {
                            PrimitiveType::F32 => "f4",
                            PrimitiveType::F64 => "f8",
                            PrimitiveType::I32 => "i4",
                            PrimitiveType::I64 => "i8",
                            PrimitiveType::U32 => "u4",
                            PrimitiveType::U64 => "u8",
                            PrimitiveType::Bool => "u1",
                            PrimitiveType::Vec3 => {
                                let vec3_col = self.at_offset(field.offset, "struct")?;
                                let viewcolumn_accessors =
                                    py.import("pybevy.ecs.view_accessors")?;
                                let vec3_wrapper =
                                    viewcolumn_accessors.getattr("Vec3ViewColumn")?;
                                return Ok(vec3_wrapper.call1((vec3_col,))?.into());
                            }
                            PrimitiveType::Vec2 => {
                                let vec2_col = self.at_offset(field.offset, "struct")?;
                                let viewcolumn_accessors =
                                    py.import("pybevy.ecs.view_accessors")?;
                                let vec2_wrapper =
                                    viewcolumn_accessors.getattr("Vec2ViewColumn")?;
                                return Ok(vec2_wrapper.call1((vec2_col,))?.into());
                            }
                        };

                        let field_col = self.at_offset(field.offset, dtype)?;
                        return Ok(Py::new(py, field_col)?.into());
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
            // Transform fields (Bevy layout: rotation @ 0, translation @ 16, scale @ 28)
            "rotation" => {
                let quat_col = self.at_offset(0, "struct")?;
                let quat_wrapper = viewcolumn_accessors.getattr("QuatViewColumn")?;
                Ok(quat_wrapper.call1((quat_col,))?.into())
            }
            "translation" => {
                let vec3_col = self.at_offset(16, "struct")?;
                let vec3_wrapper = viewcolumn_accessors.getattr("Vec3ViewColumn")?;
                Ok(vec3_wrapper.call1((vec3_col,))?.into())
            }
            "scale" => {
                let vec3_col = self.at_offset(28, "struct")?;
                let vec3_wrapper = viewcolumn_accessors.getattr("Vec3ViewColumn")?;
                Ok(vec3_wrapper.call1((vec3_col,))?.into())
            }
            // Vec3 fields
            "x" => {
                let x_col = self.at_offset(0, "f4")?;
                Ok(Py::new(py, x_col)?.into())
            }
            "y" => {
                let y_col = self.at_offset(4, "f4")?;
                Ok(Py::new(py, y_col)?.into())
            }
            "z" => {
                let z_col = self.at_offset(8, "f4")?;
                Ok(Py::new(py, z_col)?.into())
            }
            // Quat w component (x, y, z already handled above)
            "w" => {
                let w_col = self.at_offset(12, "f4")?;
                Ok(Py::new(py, w_col)?.into())
            }
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
                self.len, self.stride, self.dtype
            )
        } else {
            format!(
                "ViewColumn(len={}, stride={}, dtype='{}', valid=False [STALE])",
                self.len, self.stride, self.dtype
            )
        }
    }

    /// Support indexing for Numba JIT compatibility.
    fn __getitem__(&self, index: isize) -> PyResult<f64> {
        if !self.is_valid() {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }

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
        if !self.is_valid() {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }

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
        let elem_size = Self::dtype_size(&self.dtype)?;
        let mut buf = vec![0u8; self.len * elem_size];
        for i in 0..self.len {
            let src_addr = self.ptr + i * self.stride;
            let dst_offset = i * elem_size;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src_addr as *const u8,
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
        let elem_size = Self::dtype_size(&self.dtype)?;
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
            let dst_addr = self.ptr + i * self.stride;
            let src_offset = i * elem_size;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data[src_offset..].as_ptr(),
                    dst_addr as *mut u8,
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
        if !self.validity_token.load(Ordering::Relaxed) {
            return Err(PyRuntimeError::new_err("Accessing stale ViewColumn!"));
        }

        if let Ok(col) = value.cast::<PyViewColumn>() {
            let src = col.borrow();
            if !src.validity_token.load(Ordering::Relaxed) {
                return Err(PyRuntimeError::new_err("Source ViewColumn is stale!"));
            }
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
