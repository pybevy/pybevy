//! Lazy wrapper proxy for efficient Query[Mut[T]] iteration with wrapper storage.
//!
//! This module provides a proxy object that holds a pointer to wrapper bytes in the ECS
//! and lazily deserializes/serializes fields on access, avoiding the overhead of full
//! object deserialization on every iteration.
//!
//! ## Design: Cache-Free Architecture
//!
//! Fields are deserialized on every access without caching. This design choice:
//! - **Eliminates TLS corruption**: No `Py<PyAny>` objects stored = no Python refcount issues
//! - **Prevents crashes at scale**: Tested with 25,600 entities without issues
//! - **Minimal performance cost**: Fields rarely accessed >1× per iteration
//! - **Thread-safe**: No shared state beyond the raw pointer and layout
//!
//! Previous versions cached deserialized Python objects, which caused thread-local storage
//! corruption when used at scale (>25K entities) on Bevy's parallel worker threads.

use std::sync::Arc;

use bevy::{
    ecs::{component::ComponentId, entity::Entity, world::World},
    math::{Vec2, Vec3},
};
use pybevy_core::{
    ValueStorage,
    storage_traits::{BorrowableStorage, FromBorrowedStorage},
};
use pybevy_math::{vec2::PyVec2, vec3::PyVec3};
use pyo3::{
    exceptions::{PyAttributeError, PyRuntimeError},
    prelude::*,
    types::PyType,
};

use super::{
    component_layout::{ComponentLayout, PrimitiveType},
    helpers::validity_guard::ValidityFlagWithMode,
};

/// Lazy proxy for wrapper storage components in Query iteration.
///
/// This proxy holds a pointer to the raw wrapper bytes in the ECS and deserializes
/// fields on-demand when accessed. Mutations are immediately written back to the
/// raw bytes.
///
/// # Safety
/// The data_ptr must remain valid for the lifetime of this object. This is ensured
/// by the ValidityFlag which is invalidated when the query iteration advances.
#[pyclass(name = "LazyWrapperProxy")]
pub struct PyLazyWrapperProxy {
    /// Pointer to the raw wrapper bytes in the ECS
    data_ptr: *mut u8,

    /// Component layout (field names, types, and byte offsets)
    layout: Arc<ComponentLayout>,

    /// Python type object for this component (for __class__ attribute)
    py_type: *const pyo3::ffi::PyTypeObject,

    /// Validity flag to ensure safe access (includes mode for read-only vs mutable)
    validity: ValidityFlagWithMode,

    /// Whether this proxy allows mutations (true for Mut[T], false for read-only)
    mutable: bool,

    /// Component ID for change tracking
    component_id: ComponentId,

    /// Entity this component belongs to (for change tracking outside iteration context)
    entity: Entity,

    /// World pointer (for change tracking outside iteration context)
    world_ptr: *mut World,
}

// SAFETY: PyLazyWrapperProxy is Send because:
// - data_ptr points to ECS data which is Send (Bevy ensures this)
// - layout is Arc (Send + Sync)
// - validity is ValidityFlagWithMode (Send)
// - py_type is a static type object pointer (safe to send)
// - component_id is Copy (Send)
// - No Python objects (Py<PyAny>) are stored, eliminating TLS issues
unsafe impl Send for PyLazyWrapperProxy {}

// SAFETY: PyLazyWrapperProxy is Sync because:
// - Access to data_ptr is protected by validity checks
// - layout is Arc (Sync)
// - validity is ValidityFlagWithMode (Sync)
// - py_type is a static type object pointer (safe to share)
// - component_id is Copy (Sync)
// - No Python objects (Py<PyAny>) are stored, eliminating refcount issues
unsafe impl Sync for PyLazyWrapperProxy {}

impl PyLazyWrapperProxy {
    /// Create a new lazy wrapper proxy
    ///
    /// # Safety
    /// - data_ptr must point to valid wrapper bytes for the component's lifetime
    /// - validity must be properly invalidated when the data becomes invalid
    pub unsafe fn new(
        data_ptr: *mut u8,
        layout: Arc<ComponentLayout>,
        py_type: *const pyo3::ffi::PyTypeObject,
        validity: ValidityFlagWithMode,
        mutable: bool,
        component_id: ComponentId,
        entity: Entity,
        world_ptr: *mut World,
    ) -> Self {
        Self {
            data_ptr,
            layout,
            py_type,
            validity,
            mutable,
            component_id,
            entity,
            world_ptr,
        }
    }

    /// Check validity before any access
    #[inline]
    fn check_valid(&self) -> PyResult<()> {
        Ok(self.validity.check()?)
    }

    /// Deserialize a single field from the wrapper bytes
    fn deserialize_field(&self, py: Python, field_name: &str) -> PyResult<Py<PyAny>> {
        self.check_valid()?;

        // Find the field in the layout
        let field = self
            .layout
            .fields
            .iter()
            .find(|f| f.name == field_name)
            .ok_or_else(|| {
                let available: Vec<&str> =
                    self.layout.fields.iter().map(|f| f.name.as_str()).collect();
                PyAttributeError::new_err(format!(
                    "Component has no field '{}' (available: {})",
                    field_name,
                    available.join(", ")
                ))
            })?;

        // Read bytes at the field's offset
        let bytes_ptr = unsafe { self.data_ptr.add(field.offset) };

        // Deserialize based on field type
        let value: Py<PyAny> = match field.field_type {
            PrimitiveType::F32 => {
                let val = unsafe { *(bytes_ptr as *const f32) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::F64 => {
                let val = unsafe { *(bytes_ptr as *const f64) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::I32 => {
                let val = unsafe { *(bytes_ptr as *const i32) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::I64 => {
                let val = unsafe { *(bytes_ptr as *const i64) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::U32 => {
                let val = unsafe { *(bytes_ptr as *const u32) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::U64 => {
                let val = unsafe { *(bytes_ptr as *const u64) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::Bool => {
                let val = unsafe { *(bytes_ptr as *const bool) };
                Py::from(pyo3::types::PyBool::new(py, val)).into()
            }
            PrimitiveType::Vec3 => {
                let vec3_ptr = bytes_ptr as *mut Vec3;
                let storage = unsafe { ValueStorage::borrowed(vec3_ptr, self.validity.clone()) };
                let py_vec3 = PyVec3::from_borrowed(storage);
                Py::new(py, py_vec3)?.into_any()
            }
            PrimitiveType::Vec2 => {
                let vec2_ptr = bytes_ptr as *mut Vec2;
                let storage = unsafe { ValueStorage::borrowed(vec2_ptr, self.validity.clone()) };
                let py_vec2 = PyVec2::from_borrowed(storage);
                Py::new(py, py_vec2)?.into_any()
            }
        };

        Ok(value)
    }

    /// Serialize a single field value and write it back to the wrapper bytes
    fn serialize_field(
        &self,
        _py: Python,
        field_name: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.check_valid()?;

        if !self.mutable {
            return Err(PyRuntimeError::new_err(
                "Cannot mutate read-only component (use Mut[T] in query)",
            ));
        }

        // Find the field in the layout
        let field = self
            .layout
            .fields
            .iter()
            .find(|f| f.name == field_name)
            .ok_or_else(|| {
                PyAttributeError::new_err(format!("Component has no field '{}'", field_name))
            })?;

        // Get pointer to the field's bytes
        let bytes_ptr = unsafe { self.data_ptr.add(field.offset) };

        // Serialize based on field type
        match field.field_type {
            PrimitiveType::F32 => {
                let val: f32 = value.extract()?;
                unsafe { *(bytes_ptr as *mut f32) = val };
            }
            PrimitiveType::F64 => {
                let val: f64 = value.extract()?;
                unsafe { *(bytes_ptr as *mut f64) = val };
            }
            PrimitiveType::I32 => {
                let val: i32 = value.extract()?;
                unsafe { *(bytes_ptr as *mut i32) = val };
            }
            PrimitiveType::I64 => {
                let val: i64 = value.extract()?;
                unsafe { *(bytes_ptr as *mut i64) = val };
            }
            PrimitiveType::U32 => {
                let val: u32 = value.extract()?;
                unsafe { *(bytes_ptr as *mut u32) = val };
            }
            PrimitiveType::U64 => {
                let val: u64 = value.extract()?;
                unsafe { *(bytes_ptr as *mut u64) = val };
            }
            PrimitiveType::Bool => {
                let val: bool = value.extract()?;
                unsafe { *(bytes_ptr as *mut bool) = val };
            }
            PrimitiveType::Vec3 => {
                let py_vec3: PyRef<PyVec3> = value.extract()?;
                let v: Vec3 = (&*py_vec3).into();
                unsafe { *(bytes_ptr as *mut Vec3) = v };
            }
            PrimitiveType::Vec2 => {
                let py_vec2: PyRef<PyVec2> = value.extract()?;
                let v: Vec2 = (&*py_vec2).into();
                unsafe { *(bytes_ptr as *mut Vec2) = v };
            }
        }

        Ok(())
    }
}

#[pymethods]
impl PyLazyWrapperProxy {
    /// Get a field value (lazy deserialization), falling back to Python type methods.
    ///
    /// Priority: ECS field → Python class attribute (methods, properties, etc.)
    /// For methods, binds them to `self` so `self.field` works in the method body.
    fn __getattr__(self_: &Bound<'_, Self>, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        let this = self_.borrow();

        // Check validity first — stale proxies must always error
        this.check_valid()?;

        // Try ECS field first — deserialize_field does its own field lookup
        match this.deserialize_field(py, name) {
            Ok(value) => return Ok(value),
            Err(e) => {
                // Only fall through for AttributeError (field not found).
                // Re-raise any other error (e.g., validity/type errors).
                if !e.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) {
                    return Err(e);
                }
            }
        }

        // Fall back to Python type attributes (methods, class variables, etc.)
        let py_type =
            unsafe { pyo3::Bound::from_borrowed_ptr(py, this.py_type as *mut pyo3::ffi::PyObject) };

        // Look up the attribute on the Python type
        let attr = py_type.getattr(name).map_err(|_| {
            let available: Vec<&str> = this.layout.fields.iter().map(|f| f.name.as_str()).collect();
            pyo3::exceptions::PyAttributeError::new_err(format!(
                "Component has no field or method '{}' (fields: {})",
                name,
                available.join(", ")
            ))
        })?;

        // If it's callable, bind it to self so self.field works inside the method
        if attr.is_callable() {
            let types_module = py.import("types")?;
            let method_type = types_module.getattr("MethodType")?;
            let bound_method = method_type.call1((&attr, self_))?;
            Ok(bound_method.unbind())
        } else {
            Ok(attr.unbind())
        }
    }

    /// Set a field value (immediate writeback)
    fn __setattr__(&self, py: Python, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        // Don't intercept special Python attributes
        if name.starts_with("__") && name.ends_with("__") {
            return Err(PyAttributeError::new_err(format!(
                "Cannot set special attribute '{}'",
                name
            )));
        }

        // Serialize and write back immediately
        self.serialize_field(py, name, &value)?;

        // Mark component as changed using stored entity context
        crate::ecs::change_tracking::mark_component_changed_explicit(
            self.entity,
            self.world_ptr,
            self.component_id,
        );

        Ok(())
    }

    /// Return the component's Python type for isinstance() checks
    #[getter(__class__)]
    fn get_class(&self, py: Python) -> PyResult<Py<PyType>> {
        let py_type =
            unsafe { pyo3::Bound::from_borrowed_ptr(py, self.py_type as *mut pyo3::ffi::PyObject) };

        if let Ok(type_obj) = py_type.cast::<PyType>() {
            Ok(type_obj.clone().unbind())
        } else {
            Err(PyRuntimeError::new_err("Invalid component type"))
        }
    }

    /// String representation for debugging
    fn __repr__(&self, py: Python) -> PyResult<String> {
        self.check_valid()?;

        let type_name = self.get_class(py)?;
        let type_name_str = type_name.bind(py).name()?;

        // Deserialize all fields and format like a dataclass
        let mut field_strs = Vec::new();
        for field in &self.layout.fields {
            let value = self.deserialize_field(py, &field.name)?;
            let value_repr = value.bind(py).repr()?;
            field_strs.push(format!("{}={}", field.name, value_repr));
        }

        Ok(format!("{}({})", type_name_str, field_strs.join(", ")))
    }
}
