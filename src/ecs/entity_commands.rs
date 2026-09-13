use std::fmt;

use bevy::ecs::entity::Entity;
use pybevy_core::{ensure_no_live_asset_access, extract_entity_from_any};
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::PyTuple,
};

use super::{
    PyChildOf, PyEntity,
    commands::{PyCommands, report_deferred_error},
    helpers::validity_guard::ValidityFlag,
};
use crate::ecs::observer_registry::ObserverRegistry;

/// Represents a handle to perform deferred operations on an entity.
/// Operations are queued and applied later when the Commands are flushed.
#[pyclass(name = "EntityCommands", module = "pybevy.ecs", skip_from_py_object)]
pub struct PyEntityCommands {
    pub(crate) id: Entity,
    commands: Option<PyCommands>,
    // Runtime validity check - prevents use after system execution
    // This is cloned from the parent PyCommands/PyWorld
    validity: Option<ValidityFlag>,
}

impl Clone for PyEntityCommands {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            commands: self
                .commands
                .as_ref()
                .map(|commands| Python::attach(|py| commands.clone_for_handle(py))),
            validity: self.validity.clone(),
        }
    }
}

impl fmt::Debug for PyEntityCommands {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PyEntityCommands")
            .field("id", &self.id)
            .finish()
    }
}

impl PyEntityCommands {
    pub(crate) fn with_commands(entity: Entity, commands: &PyCommands, py: Python<'_>) -> Self {
        Self {
            id: entity,
            commands: Some(commands.clone_for_handle(py)),
            validity: Some(commands.validity()),
        }
    }

    pub(crate) fn with_world(entity: Entity, world: PyRef<'_, super::world::PyWorld>) -> Self {
        let world_ptr = world.world_ptr();
        let validity = world.validity().unwrap_or_default();
        let owner = world.into();
        // SAFETY: the retained owner keeps the World allocated; validity gates access.
        let commands = unsafe { PyCommands::from_world(world_ptr, owner, validity.clone()) };
        Self {
            id: entity,
            commands: Some(commands),
            validity: Some(validity),
        }
    }

    /// Check if this EntityCommands instance is still valid for use
    fn check_valid(&self) -> PyResult<()> {
        if let Some(ref validity) = self.validity {
            Ok(validity.check()?)
        } else {
            Ok(()) // No validity tracking (e.g., simple entity ID returns or owned worlds)
        }
    }

    fn get_commands(&self) -> PyResult<Option<&PyCommands>> {
        self.check_valid()?;
        Ok(self.commands.as_ref())
    }
}

#[pymethods]
impl PyEntityCommands {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(commands) = &self.commands {
            commands.traverse_owner(visit)?;
        }
        Ok(())
    }

    /// Get the entity ID
    pub fn id(&self) -> PyEntity {
        PyEntity(self.id)
    }

    /// Insert components into this entity
    #[pyo3(signature = (*components))]
    pub fn insert(
        &self,
        py: Python,
        components: &Bound<'_, PyTuple>,
    ) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands()? {
            crate::ecs::commands::insert_components_to_entity_helper(
                source, py, self.id, components,
            )?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot insert components: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Remove components from this entity
    #[pyo3(signature = (*components))]
    pub fn remove(
        &self,
        py: Python,
        components: &Bound<'_, PyTuple>,
    ) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands()? {
            crate::ecs::commands::remove_components_from_entity_helper(
                source, py, self.id, components,
            )?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot remove components: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Trigger an event for this entity.
    pub fn trigger(&self, py: Python, event: Bound<'_, PyAny>) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands()? {
            crate::ecs::commands::trigger_event_helper(source, py, event, Some(self.id))?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot trigger event: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Despawn this entity
    pub fn despawn(&self) -> PyResult<()> {
        if let Some(source) = self.get_commands()? {
            source.despawn_entity(&PyEntity(self.id))
        } else {
            Err(PyValueError::new_err(
                "Cannot despawn: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Add a child entity to this entity
    pub fn add_child(&self, child: &Bound<'_, PyAny>) -> PyResult<PyEntityCommands> {
        let child = extract_entity_from_any(child)?;
        if let Some(source) = self.get_commands()? {
            crate::ecs::commands::add_child_helper(source, self.id, child.0)?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot add child: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Set the parent of this entity
    pub fn set_parent(&self, parent: &Bound<'_, PyAny>) -> PyResult<PyEntityCommands> {
        let parent = extract_entity_from_any(parent)?;
        if let Some(source) = self.get_commands()? {
            crate::ecs::commands::set_parent_helper(source, self.id, parent.0)?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot set parent: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Remove the parent relationship from this entity
    pub fn remove_parent(&self) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands()? {
            crate::ecs::commands::remove_parent_helper(source, self.id)?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot remove parent: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Remove specific children from this entity
    #[pyo3(signature = (*children))]
    pub fn remove_children(
        &self,
        children: &Bound<'_, pyo3::types::PyTuple>,
    ) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands()? {
            let child_ids: Vec<Entity> = children
                .iter()
                .map(|item| {
                    item.extract::<PyEntity>()
                        .map(|e| e.0)
                        .map_err(|_| PyTypeError::new_err("Expected Entity objects"))
                })
                .collect::<PyResult<Vec<_>>>()?;

            crate::ecs::commands::remove_children_helper(source, self.id, &child_ids)?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot remove children: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Remove all children from this entity
    pub fn clear_children(&self) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands()? {
            crate::ecs::commands::clear_children_helper(source, self.id)?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot clear children: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Spawn children entities using a callback function
    pub fn with_children(&self, py: Python, func: Bound<'_, PyAny>) -> PyResult<PyEntityCommands> {
        let ty = func.get_type();

        if !ty.is_callable() {
            return Err(PyValueError::new_err("Parameter must be callable"));
        }

        let spawner = if let Some(commands) = self.get_commands()? {
            PyRelatedSpawnerCommands::with_commands(self.id, commands, py)
        } else {
            return Err(PyValueError::new_err(
                "Cannot spawn children: EntityCommands not associated with a Commands or World object.",
            ));
        };

        let related_spawner = Py::new(py, spawner)?;
        func.call1((related_spawner,))?;

        Ok(self.clone())
    }

    /// Register an observer for this specific entity
    ///
    /// The observer will only trigger when events target this entity.
    ///
    /// # Example
    /// ```python
    /// def on_damage(trigger: On[TakeDamage]) -> None:
    ///     print(f"Entity {trigger.entity()} took damage")
    ///
    /// commands.spawn(Player()).observe(on_damage)
    /// ```
    pub fn observe(&self, py: Python, observer: Bound<'_, PyAny>) -> PyResult<PyEntityCommands> {
        // Try to get world access from either Commands or World
        let mut world_guard = if let Some(commands) = self.get_commands()? {
            // Via Commands (immediate mode only)
            commands.try_world_mut()?
        } else {
            None
        };

        if let Some(world) = world_guard.as_deref_mut() {
            // Immediate registration - we have World access
            ensure_no_live_asset_access(world, "entity.observe()")
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let _observer_entity =
                ObserverRegistry::register_observer_for_entity(py, &observer, self.id, world)?;
            Ok(self.clone())
        } else if let Some(commands) = self.get_commands()? {
            // Deferred registration - queue a command
            ObserverRegistry::validate_observer_signature(py, &observer)?;
            let entity_id = self.id;
            let observer_py: Py<PyAny> = observer.unbind();
            let error_sink = commands.error_sink();

            commands.execute_or_queue(move |world| {
                Python::attach(|py| {
                    let observer_bound = observer_py.bind(py);
                    if let Err(e) = ObserverRegistry::register_observer_for_entity(
                        py,
                        observer_bound,
                        entity_id,
                        world,
                    ) {
                        report_deferred_error(
                            &error_sink,
                            "Failed to register observer via deferred command",
                            e,
                        );
                    }
                });
            })?;

            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "EntityCommands.observe() requires either World or Commands access.",
            ))
        }
    }
}

/// Helper for spawning entities related to a target entity.
#[pyclass(name = "RelatedSpawnerCommands", module = "pybevy.ecs")]
pub struct PyRelatedSpawnerCommands {
    target: Entity,
    commands: PyCommands,
}

impl PyRelatedSpawnerCommands {
    fn with_commands(target: Entity, commands: &PyCommands, py: Python<'_>) -> Self {
        Self {
            target,
            commands: commands.clone_for_handle(py),
        }
    }

    fn commands_source(&self) -> PyResult<&PyCommands> {
        self.commands.validity().check()?;
        Ok(&self.commands)
    }

    /// Create a ChildOf component for the target entity
    fn create_child_of_component(py: Python, target: Entity) -> PyResult<Py<PyAny>> {
        let child_of = Py::new(py, PyChildOf::new(PyEntity(target)))?;
        Ok(child_of.into_any())
    }
}

#[pymethods]
impl PyRelatedSpawnerCommands {
    #[new]
    pub fn new(py: Python, commands: Py<PyCommands>, target: PyEntity) -> PyResult<Self> {
        let commands_ref = commands.bind(py).borrow();
        Ok(Self::with_commands(target.0, &commands_ref, py))
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        self.commands.traverse_owner(visit)
    }

    /// Spawn an empty entity as a child
    pub fn spawn_empty(&self, py: Python) -> PyResult<PyEntityCommands> {
        let source = self.commands_source()?;
        let entity_cmd = source.spawn_empty(py)?;

        // Insert ChildOf component to establish parent-child relationship
        let child_of = Self::create_child_of_component(py, self.target)?;
        let child_of_tuple = PyTuple::new(py, vec![child_of])?;
        entity_cmd.insert(py, &child_of_tuple)?;

        Ok(entity_cmd)
    }

    /// Spawn an entity with components as a child
    #[pyo3(signature = (*components))]
    pub fn spawn(&self, py: Python, components: &Bound<'_, PyTuple>) -> PyResult<PyEntityCommands> {
        let source = self.commands_source()?;
        let entity_cmd = source.spawn(py, components)?;

        // Insert ChildOf component to establish parent-child relationship
        let child_of = Self::create_child_of_component(py, self.target)?;
        let child_of_tuple = PyTuple::new(py, vec![child_of])?;
        entity_cmd.insert(py, &child_of_tuple)?;

        Ok(entity_cmd)
    }

    /// Get the target entity ID
    pub fn target_entity(&self) -> PyEntity {
        PyEntity(self.target)
    }
}
