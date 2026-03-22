//! Custom component storage and field access for Python-defined components.
//!
//! This module implements field access for custom Python components decorated with `@component`.
//! Components are stored as borrowed pointers to PyObject in Bevy's ECS (returned from queries).
//!
//! # Safety
//!
//! Borrowed references use ValidityFlag to ensure pointers are only dereferenced during
//! system execution. After system completes, ValidityFlag is invalidated and any access
//! to borrowed components will raise a runtime error.

use bevy::ecs::{entity::Entity, world::World};
use pyo3::{prelude::*, types::PyAny};

use crate::ecs::{component::PyComponent, helpers::validity_guard::ValidityFlagWithMode};

/// Storage for custom Python components - borrowed pointer to ECS-managed Python object
struct CustomComponentStorage {
    ptr: *mut pyo3::ffi::PyObject,
    validity: ValidityFlagWithMode,
    component_id: bevy::ecs::component::ComponentId,
    /// Entity this component belongs to (for change tracking outside iteration context)
    entity: Entity,
    /// World pointer (for change tracking outside iteration context)
    world_ptr: *mut World,
}

// SAFETY: PyObject pointers are stable (GC doesn't move objects),
// ValidityFlag ensures pointer lifetime is bounded by system execution,
// entity and world_ptr are only dereferenced when ValidityFlag is valid
unsafe impl Send for CustomComponentStorage {}
unsafe impl Sync for CustomComponentStorage {}

/// Python wrapper for custom component instances
///
/// This type wraps custom Python components (created with `@component` decorator)
/// and provides field access via `__getattribute__` and `__setattr__`.
///
/// # Examples
///
/// ```python
/// @component
/// class Health(Component):
///     value: float = 100.0
///
/// # In Query:
/// for health in query:
///     health.value += 10.0  # Calls __getattribute__ then __setattr__
/// ```
#[pyclass(name = "CustomComponent", extends = PyComponent)]
pub struct PyCustomComponent {
    storage: CustomComponentStorage,
}

#[pymethods]
impl PyCustomComponent {
    /// Intercept field access to forward to underlying Python object
    fn __getattribute__<'py>(
        slf: PyRef<'py, Self>,
        py: Python<'py>,
        name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Check validity first
        slf.check_valid()?;

        // Get underlying Python object
        let py_obj = slf.get_py_object(py);

        // Forward to Python object's __getattribute__
        py_obj.getattr(name)
    }

    /// Intercept field mutation to forward to underlying Python object
    fn __setattr__(
        slf: PyRefMut<'_, Self>,
        py: Python<'_>,
        name: &str,
        value: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        slf.check_valid()?;

        // Ensure mutable access
        slf.storage.validity.check_write()?;

        // Get underlying Python object
        let py_obj = slf.get_py_object(py);

        // Forward to Python object's __setattr__
        py_obj.setattr(name, value)?;

        // Mark component as changed using stored entity context.
        // This works both during iteration and after collection (list(query)).
        crate::ecs::change_tracking::mark_component_changed_explicit(
            slf.storage.entity,
            slf.storage.world_ptr,
            slf.storage.component_id,
        );

        Ok(())
    }
}

impl PyCustomComponent {
    /// Create borrowed reference to ECS-stored component
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - `py_obj_ptr` points to a valid PyObject for the duration of `validity`
    /// - ValidityFlag is invalidated when system execution completes
    /// - `component_id` is the correct ComponentId for this component type
    /// - `entity` is valid in the world for the duration of `validity`
    /// - `world_ptr` points to a valid World for the duration of `validity`
    pub fn from_borrowed(
        py_obj_ptr: *mut pyo3::ffi::PyObject,
        validity: ValidityFlagWithMode,
        component_id: bevy::ecs::component::ComponentId,
        entity: Entity,
        world_ptr: *mut World,
    ) -> Self {
        PyCustomComponent {
            storage: CustomComponentStorage {
                ptr: py_obj_ptr,
                validity,
                component_id,
                entity,
                world_ptr,
            },
        }
    }

    /// Check if this component reference is still valid
    fn check_valid(&self) -> PyResult<()> {
        Ok(self.storage.validity.check()?)
    }

    /// Get the underlying Python object
    fn get_py_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        // SAFETY: ValidityFlag ensures ptr is valid during system execution
        // from_borrowed_ptr creates a reference without incrementing refcount
        unsafe { Bound::from_borrowed_ptr(py, self.storage.ptr) }
    }
}
