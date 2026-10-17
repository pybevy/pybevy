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
    ecs::{
        component::ComponentId,
        entity::Entity,
        world::{World, unsafe_world_cell::UnsafeWorldCell},
    },
    math::{Mat2, Mat3, Mat4, Quat, Vec2, Vec3, Vec4},
};
use pybevy_core::{ComponentWriteContext, ValueStorage, storage_traits::FromBorrowedStorage};
use pybevy_ecs::shared::run_ticks::RunTicks;
use pybevy_math::{
    mat2::PyMat2, mat3::PyMat3, mat4::PyMat4, quat::PyQuat, vec2::PyVec2, vec3::PyVec3,
    vec4::PyVec4,
};
use pyo3::{
    PyClass, PyClassInitializer, PyTraverseError, PyVisit,
    exceptions::{PyAttributeError, PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyDict, PyType},
};

use super::{
    component_layout::{ComponentLayout, PrimitiveType, extract_primitive_from_py},
    helpers::validity_guard::ValidityFlagWithMode,
};

/// How a [`PyLazyWrapperProxy`] relates to ECS storage, which decides whether its cached
/// `data_ptr` can be trusted across a structural mutation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProxyKind {
    /// Long-lived handle from `world.get` / `world.get_mut`. The entity may be
    /// structurally mutated while the proxy is held, so the address is re-resolved from
    /// `(world_ptr, entity, component_id)` on every access, and escaped composite field
    /// handles (vectors, quaternions, matrices) re-resolve too.
    WorldGet,
    /// Query iteration item. The cached `data_ptr` is valid for the ValidityFlag window
    /// (no immediate structural mutation can intervene), so it is used directly.
    QueryItem,
}

/// Lazy proxy for wrapper storage components in Query iteration.
///
/// This proxy holds a pointer to the raw wrapper bytes in the ECS and deserializes
/// fields on-demand when accessed. Mutations are immediately written back to the
/// raw bytes.
///
/// # Safety
/// For query-iteration proxies (`ProxyKind::QueryItem`) the `data_ptr` must remain valid
/// for the lifetime of this object; this is ensured by the ValidityFlag which is
/// invalidated when the query iteration advances. For long-lived proxies returned from
/// `world.get` / `world.get_mut` (`ProxyKind::WorldGet`) the ValidityFlag does not cover
/// structural mutations, so `data_ptr` is treated as a stale hint and the current
/// address is re-resolved from `(world_ptr, entity, component_id)` on every access.
#[pyclass(name = "LazyWrapperProxy", module = "pybevy.ecs")]
pub struct PyLazyWrapperProxy {
    /// Pointer to the raw wrapper bytes in the ECS.
    ///
    /// Only trusted directly on the query fast path (`ProxyKind::QueryItem`). For
    /// `ProxyKind::WorldGet` this is a construction-time hint and may dangle after a
    /// structural mutation; `resolved_ptr()` re-resolves instead.
    data_ptr: *mut u8,

    /// Component layout (field names, types, and byte offsets)
    layout: Arc<ComponentLayout>,

    /// Python type object for this component (for __class__ and method lookup).
    /// Owning the reference keeps the class valid after its query param drops.
    py_type: Option<Py<PyType>>,

    /// Validity flag to ensure safe access (includes mode for read-only vs mutable)
    validity: ValidityFlagWithMode,

    /// Whether this proxy allows mutations (true for Mut[T], false for read-only)
    mutable: bool,

    /// Component ID for change tracking
    component_id: ComponentId,

    /// Entity this component belongs to (for change tracking outside iteration context)
    entity: Entity,

    /// World pointer used only to re-resolve long-lived `WorldGet` proxies.
    /// Query items use their cached data pointer and store null here.
    world_ptr: *mut World,

    /// The validity-fenced world cell used for narrow change-tick writeback.
    world_cell: UnsafeWorldCell<'static>,

    /// Captured query ticks. `None` is used by long-lived `World.get_mut`
    /// proxies, which intentionally stamp Bevy's current live tick.
    run_ticks: Option<RunTicks>,

    /// Whether this proxy re-resolves per access (`WorldGet`) or trusts the cached
    /// `data_ptr` for the ValidityFlag window (`QueryItem`).
    kind: ProxyKind,
}

// SAFETY: PyLazyWrapperProxy is Send because:
// - data_ptr points to ECS data which is Send (Bevy ensures this)
// - layout is Arc (Send + Sync)
// - validity is ValidityFlagWithMode (Send)
// - Py<PyType> follows PyO3's cross-thread ownership rules
// - component_id is Copy (Send)
// - the retained Py<PyType> is an owned, GIL-independent handle
unsafe impl Send for PyLazyWrapperProxy {}

// SAFETY: PyLazyWrapperProxy is Sync because:
// - Access to data_ptr is protected by validity checks
// - layout is Arc (Sync)
// - validity is ValidityFlagWithMode (Sync)
// - Py<PyType> follows PyO3's cross-thread ownership rules
// - component_id is Copy (Sync)
// - the retained Py<PyType> is an owned, GIL-independent handle
unsafe impl Sync for PyLazyWrapperProxy {}

impl PyLazyWrapperProxy {
    /// Create a new lazy wrapper proxy
    ///
    /// # Safety
    /// - data_ptr must point to valid wrapper bytes for the component's lifetime
    /// - validity must be properly invalidated when the data becomes invalid
    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe fn new(
        data_ptr: *mut u8,
        layout: Arc<ComponentLayout>,
        py_type: *const pyo3::ffi::PyTypeObject,
        validity: ValidityFlagWithMode,
        mutable: bool,
        component_id: ComponentId,
        entity: Entity,
        world_ptr: *mut World,
        world_cell: UnsafeWorldCell<'_>,
        run_ticks: Option<RunTicks>,
        kind: ProxyKind,
    ) -> Self {
        let py_type = if py_type.is_null() {
            None
        } else {
            Some(Python::attach(|py| {
                // SAFETY: the caller supplied a live component class pointer.
                unsafe {
                    Bound::from_borrowed_ptr(py, py_type.cast_mut().cast::<pyo3::ffi::PyObject>())
                }
                .cast_into::<PyType>()
                .expect("component type pointer must name a Python type")
                .unbind()
            }))
        };
        // SAFETY: the caller fences this lifetime-erased cell with `validity`.
        let world_cell = unsafe {
            std::mem::transmute::<UnsafeWorldCell<'_>, UnsafeWorldCell<'static>>(world_cell)
        };
        Self {
            data_ptr,
            layout,
            py_type,
            validity,
            mutable,
            component_id,
            entity,
            world_ptr,
            world_cell,
            run_ticks,
            kind,
        }
    }

    /// Check validity before any access
    #[inline]
    fn check_valid(&self) -> PyResult<()> {
        Ok(self.validity.check()?)
    }

    fn class_attribute<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let py_type = self
            .py_type
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Invalid component type"))?
            .bind(py);
        py.import("inspect")?
            .call_method1("getattr_static", (py_type, name))
    }

    fn bound_class_attribute<'py>(
        &self,
        proxy: &Bound<'py, Self>,
        name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let attribute = self.class_attribute(proxy.py(), name)?;
        self.bind_class_attribute(proxy, attribute)
    }

    fn bind_class_attribute<'py>(
        &self,
        proxy: &Bound<'py, Self>,
        attribute: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let Some(getter) = attribute.getattr_opt("__get__")? else {
            return Ok(attribute);
        };
        let py_type = self
            .py_type
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Invalid component type"))?
            .bind(proxy.py());
        getter.call1((proxy, py_type))
    }

    /// Effective base pointer for the component's data bytes.
    ///
    /// For long-lived proxies (`ProxyKind::WorldGet`, from `world.get`/`world.get_mut`)
    /// the entity may have been structurally mutated since construction, so the cached
    /// `data_ptr` can dangle: re-resolve the component's current address and error if
    /// the entity was despawned or the component removed. For query-iteration proxies
    /// (`ProxyKind::QueryItem`) the cached pointer is kept valid by the ValidityFlag for
    /// the duration of the access, so it is used directly (fast path).
    ///
    /// Callers must not run Python code between calling this and using the pointer: a
    /// re-entrant structural mutation would invalidate it. Re-resolve again instead.
    #[inline]
    fn resolved_ptr(&self) -> PyResult<*const u8> {
        match self.kind {
            ProxyKind::WorldGet => {
                // SAFETY: world_ptr is valid while `validity` is active (callers run
                // check_valid first); synchronous Python attribute access holds no
                // competing &mut World.
                unsafe {
                    pybevy_ecs::shared::change_tracking::reresolve_wrapper_ptr(
                        self.entity,
                        self.world_ptr,
                        self.component_id,
                    )
                }
                .ok_or_else(|| {
                    PyRuntimeError::new_err(
                        "Component no longer available (entity despawned or component removed)",
                    )
                })
            }
            ProxyKind::QueryItem => Ok(self.data_ptr.cast_const()),
        }
    }

    /// Effective base pointer for a write, resolved with mutable authority.
    ///
    /// Same contract as [`Self::resolved_ptr`]; callers must already have
    /// checked that this proxy is writable.
    #[inline]
    fn resolved_write_ptr(&self) -> PyResult<*mut u8> {
        match self.kind {
            ProxyKind::WorldGet => {
                // SAFETY: the world cell is valid while `validity` is active, the
                // proxy holds write access, and no other reference to this
                // component is live during synchronous attribute assignment.
                unsafe {
                    pybevy_ecs::shared::change_tracking::reresolve_wrapper_ptr_mut(
                        self.entity,
                        self.world_cell,
                        self.component_id,
                    )
                }
                .ok_or_else(|| {
                    PyRuntimeError::new_err(
                        "Component no longer available (entity despawned or component removed)",
                    )
                })
            }
            ProxyKind::QueryItem => Ok(self.data_ptr),
        }
    }

    /// Attach lazy Bevy change tracking to a borrowed query field.
    fn query_field_validity(&self, offset: usize) -> ValidityFlagWithMode {
        let validity = self.validity.clone();
        if !self.mutable || self.kind != ProxyKind::QueryItem {
            return validity;
        }
        let Some(ticks) = self.run_ticks else {
            return validity;
        };
        // SAFETY: mutable wrapper queries declare write access to this exact
        // component, `offset` comes from its registered field layout, and the
        // proxy validity flag fences the captured world cell.
        let context = unsafe {
            ComponentWriteContext::new_with_offset(
                self.world_cell,
                self.entity,
                self.component_id,
                offset,
                ticks.last_run,
                ticks.this_run,
            )
        };
        validity.with_component_write_context(context)
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

        // Read bytes at the field's offset. For long-lived proxies this re-resolves
        // the component's current address (the cached data_ptr may dangle after a
        // structural mutation); for query iteration it is the cached pointer.
        // SAFETY: validity is checked and the registered layout bounds the live component field.
        let bytes_ptr = unsafe { self.resolved_ptr()?.add(field.offset) };

        // Deserialize based on field type
        let value: Py<PyAny> = match field.field_type {
            PrimitiveType::F32 => {
                // SAFETY: the validated layout identifies an aligned live f32 field.
                let val = unsafe { *(bytes_ptr as *const f32) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::F64 => {
                // SAFETY: the validated layout identifies an aligned live f64 field.
                let val = unsafe { *(bytes_ptr as *const f64) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::I32 => {
                // SAFETY: the validated layout identifies an aligned live i32 field.
                let val = unsafe { *(bytes_ptr as *const i32) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::I64 => {
                // SAFETY: the validated layout identifies an aligned live i64 field.
                let val = unsafe { *(bytes_ptr as *const i64) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::U32 => {
                // SAFETY: the validated layout identifies an aligned live u32 field.
                let val = unsafe { *(bytes_ptr as *const u32) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::U64 => {
                // SAFETY: the validated layout identifies an aligned live u64 field.
                let val = unsafe { *(bytes_ptr as *const u64) };
                val.into_pyobject(py)?.unbind().into()
            }
            PrimitiveType::Bool => {
                // SAFETY: the validated layout identifies a live initialized bool field.
                let val = unsafe { *(bytes_ptr as *const bool) };
                Py::from(pyo3::types::PyBool::new(py, val)).into()
            }
            PrimitiveType::Vec3 => self.composite_field::<Vec3, PyVec3>(py, field.offset)?,
            PrimitiveType::Vec2 => self.composite_field::<Vec2, PyVec2>(py, field.offset)?,
            PrimitiveType::Vec4 => self.composite_field::<Vec4, PyVec4>(py, field.offset)?,
            PrimitiveType::Quat => self.composite_field::<Quat, PyQuat>(py, field.offset)?,
            PrimitiveType::Mat2 => self.composite_field::<Mat2, PyMat2>(py, field.offset)?,
            PrimitiveType::Mat3 => self.composite_field::<Mat3, PyMat3>(py, field.offset)?,
            PrimitiveType::Mat4 => self.composite_field::<Mat4, PyMat4>(py, field.offset)?,
        };

        Ok(value)
    }

    /// Wrap a composite field as a live math value that writes through to the component.
    ///
    /// Safety contract: `bytes_ptr` is this field's current address, `offset` its layout
    /// offset, and the layout placed a `T` there at `T`'s native alignment.
    fn composite_field<T, W>(&self, py: Python, offset: usize) -> PyResult<Py<PyAny>>
    where
        T: Copy,
        W: FromBorrowedStorage<ValueStorage<T>> + PyClass + Into<PyClassInitializer<W>>,
    {
        let storage = match self.kind {
            // Long-lived proxy: hand back a re-resolving handle, not a borrow into ECS
            // storage, so `obj.field.x = ...` survives structural moves.
            // SAFETY: the proxy's world cell, entity, and component identify a live
            // wrapper whose layout holds a `T` at `offset`; `validity` fences access.
            ProxyKind::WorldGet => unsafe {
                ValueStorage::revalidating(
                    self.world_cell,
                    self.entity,
                    self.component_id,
                    offset,
                    self.validity.clone(),
                )
            },
            // Query iteration: the ValidityFlag covers the access window, so a borrowed
            // (write-through) sub-proxy is safe and preserves mutation.
            // SAFETY: the query's data pointer plus the field offset is the field's live,
            // `T`-aligned address for the validity window, and write mode follows the
            // query's access.
            ProxyKind::QueryItem => unsafe {
                ValueStorage::borrowed(
                    self.data_ptr.add(offset) as *mut T,
                    self.query_field_validity(offset),
                )
            },
        };
        Ok(Py::new(py, W::from_borrowed(storage))?.into_any())
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
        let field_type = field.field_type;
        let offset = field.offset;

        // Coerce the Python value to a plain Rust `PrimitiveValue` FIRST. `value.extract()`
        // can re-enter Python (e.g. via __float__/__index__/__bool__), and that Python code
        // may run a structural mutation which reallocates or moves this component's storage.
        // So all Python interaction must finish before we resolve a pointer: holding a
        // resolved pointer across extract() would risk a use-after-free.
        let write = extract_primitive_from_py(field_type, value)?;

        // Resolve the pointer immediately before the write, with no Python calls in
        // between. For long-lived proxies this fetches the component's current address
        // (erroring if the entity was despawned or the component removed since
        // construction); for query iteration it is the cached pointer.
        // SAFETY: writable validity and the registered layout bound the re-resolved field.
        let bytes_ptr = unsafe { self.resolved_write_ptr()?.add(offset) };
        // SAFETY: bytes_ptr is the field's current address; `write` matches field_type.
        unsafe { write.write_to_ptr(bytes_ptr) };

        Ok(())
    }
}

#[pymethods]
impl PyLazyWrapperProxy {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.py_type)
    }

    /// Get a field value (lazy deserialization), falling back to Python descriptors.
    ///
    /// Priority: ECS field → Python class descriptor.
    fn __getattr__(self_: &Bound<'_, Self>, py: Python, name: &str) -> PyResult<Py<PyAny>> {
        let this = self_.borrow();

        // Check validity first: stale proxies must always error
        this.check_valid()?;

        if name == "__dict__" {
            let values = PyDict::new(py);
            for field in &this.layout.fields {
                values.set_item(&field.name, this.deserialize_field(py, &field.name)?)?;
            }
            return Ok(values.into_any().unbind());
        }

        // Try ECS field first: deserialize_field does its own field lookup
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

        let attribute = this.class_attribute(py, name).map_err(|error| {
            if !error.is_instance_of::<PyAttributeError>(py) {
                return error;
            }
            let available: Vec<&str> = this.layout.fields.iter().map(|f| f.name.as_str()).collect();
            PyAttributeError::new_err(format!(
                "Component has no field or method '{}' (fields: {})",
                name,
                available.join(", ")
            ))
        })?;
        Ok(this.bind_class_attribute(self_, attribute)?.unbind())
    }

    /// Set a field value (immediate writeback)
    fn __setattr__(
        self_: &Bound<'_, Self>,
        py: Python,
        name: &str,
        value: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let this = self_.borrow();
        this.check_valid()?;
        // Don't intercept special Python attributes
        if name.starts_with("__") && name.ends_with("__") {
            return Err(PyAttributeError::new_err(format!(
                "Cannot set special attribute '{}'",
                name
            )));
        }

        if !this.mutable {
            return Err(PyRuntimeError::new_err(
                "Cannot mutate read-only component (use Mut[T] in query)",
            ));
        }

        if this.layout.fields.iter().any(|field| field.name == name) {
            this.serialize_field(py, name, &value)?;
        } else {
            let descriptor = this.class_attribute(py, name).map_err(|error| {
                if error.is_instance_of::<PyAttributeError>(py) {
                    PyAttributeError::new_err(format!("Component has no field '{}'", name))
                } else {
                    error
                }
            })?;
            let Some(setter) = descriptor.getattr_opt("__set__")? else {
                return Err(PyAttributeError::new_err(format!(
                    "Component has no field '{}'",
                    name
                )));
            };
            setter.call1((self_, value))?;
            this.check_valid()?;
        }

        // Mark component as changed using stored entity context.
        // SAFETY: world_ptr is valid (protected by ValidityFlag on LazyWrapperProxy),
        // entity was extracted from a live query during system execution.
        unsafe {
            pybevy_ecs::shared::change_tracking::mark_component_changed_explicit(
                this.entity,
                this.world_cell,
                this.component_id,
                this.run_ticks,
            );
        }

        Ok(())
    }

    /// Return the component's Python type for isinstance() checks
    #[getter(__class__)]
    fn get_class(&self, py: Python) -> PyResult<Py<PyType>> {
        self.py_type
            .as_ref()
            .map(|type_obj| type_obj.clone_ref(py))
            .ok_or_else(|| PyRuntimeError::new_err("Invalid component type"))
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

    fn __str__(self_: &Bound<'_, Self>) -> PyResult<String> {
        let this = self_.borrow();
        this.check_valid()?;
        match this.bound_class_attribute(self_, "__str__") {
            Ok(method) => method.call0()?.extract(),
            Err(error) if error.is_instance_of::<PyAttributeError>(self_.py()) => {
                this.__repr__(self_.py())
            }
            Err(error) => Err(error),
        }
    }

    fn __len__(self_: &Bound<'_, Self>) -> PyResult<usize> {
        let this = self_.borrow();
        this.check_valid()?;
        let method = this
            .bound_class_attribute(self_, "__len__")
            .map_err(|error| {
                if error.is_instance_of::<PyAttributeError>(self_.py()) {
                    PyTypeError::new_err("component does not define __len__")
                } else {
                    error
                }
            })?;
        method.call0()?.extract()
    }

    fn __iter__(self_: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let this = self_.borrow();
        this.check_valid()?;
        let method = this
            .bound_class_attribute(self_, "__iter__")
            .map_err(|error| {
                if error.is_instance_of::<PyAttributeError>(self_.py()) {
                    PyTypeError::new_err("component does not define __iter__")
                } else {
                    error
                }
            })?;
        Ok(method.call0()?.unbind())
    }

    fn __lt__(self_: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let this = self_.borrow();
        this.check_valid()?;
        Ok(this
            .bound_class_attribute(self_, "__lt__")?
            .call1((other,))?
            .unbind())
    }
}
