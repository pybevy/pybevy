//! Custom component storage and field access for Python-defined components.
//!
//! This module implements field access for custom Python components decorated with `@component`.
//! Components retain the Python object stored in Bevy's ECS while a proxy is alive.
//!
//! # Safety
//!
//! References use ValidityFlag to ensure the component access remains inside its declared
//! execution window. The owned Python reference prevents a removed component from leaving
//! an escaped proxy with a dangling object pointer.

use bevy::ecs::{entity::Entity, world::unsafe_world_cell::UnsafeWorldCell};
use pybevy_ecs::shared::run_ticks::RunTicks;
use pyo3::{PyTraverseError, PyVisit, prelude::*, types::PyAny};

use crate::ecs::{component::PyComponent, helpers::validity_guard::ValidityFlagWithMode};

/// Storage for a custom Python component proxy.
struct CustomComponentStorage {
    object: Py<PyAny>,
    validity: ValidityFlagWithMode,
    component_id: bevy::ecs::component::ComponentId,
    /// Entity this component belongs to (for change tracking outside iteration context)
    entity: Entity,
    /// Validity-fenced cell used for narrow change-tick writeback.
    world_cell: UnsafeWorldCell<'static>,
    /// Captured scheduled-run ticks, or `None` for `World.get_mut`.
    run_ticks: Option<RunTicks>,
}

// SAFETY: Py<PyAny> follows PyO3's cross-thread ownership rules. World access is
// fenced by ValidityFlag and pinned to its owning thread.
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
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.storage.object)
    }

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
        // SAFETY: world_ptr is valid (protected by ValidityFlag on CustomComponentProxy),
        // entity was extracted from a live query during system execution.
        unsafe {
            pybevy_ecs::shared::change_tracking::mark_component_changed_explicit(
                slf.storage.entity,
                slf.storage.world_cell,
                slf.storage.component_id,
                slf.storage.run_ticks,
            );
        }

        Ok(())
    }
}

impl PyCustomComponent {
    /// Create a proxy for an ECS-stored component.
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - ValidityFlag is invalidated when system execution completes
    /// - `component_id` is the correct ComponentId for this component type
    /// - `world_cell` references the matching World for the duration of `validity`
    pub fn from_object(
        object: Py<PyAny>,
        validity: ValidityFlagWithMode,
        component_id: bevy::ecs::component::ComponentId,
        entity: Entity,
        world_cell: UnsafeWorldCell<'_>,
        run_ticks: Option<RunTicks>,
    ) -> Self {
        // SAFETY: the caller fences this lifetime-erased cell with `validity`.
        let world_cell = unsafe {
            std::mem::transmute::<UnsafeWorldCell<'_>, UnsafeWorldCell<'static>>(world_cell)
        };
        PyCustomComponent {
            storage: CustomComponentStorage {
                object,
                validity,
                component_id,
                entity,
                world_cell,
                run_ticks,
            },
        }
    }

    /// Check if this component reference is still valid
    fn check_valid(&self) -> PyResult<()> {
        Ok(self.storage.validity.check()?)
    }

    /// Get the underlying Python object
    fn get_py_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.storage.object.bind(py).clone()
    }
}
