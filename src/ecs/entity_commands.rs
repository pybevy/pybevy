use bevy::ecs::entity::Entity;
use pybevy_core::ensure_no_live_asset_access;
use pyo3::{
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
#[derive(Debug, Clone)]
pub struct PyEntityCommands {
    pub(crate) id: Entity,
    // Store commands pointer - only valid during command queue operations
    // This will be None for simple entity ID returns
    commands_ptr: Option<usize>,
    // Store world pointer - used when spawned from World directly
    // This allows immediate operations like observe() to work
    world_ptr: Option<usize>,
    // Runtime validity check - prevents use after system execution
    // This is cloned from the parent PyCommands/PyWorld
    validity: Option<ValidityFlag>,
}

// SAFETY: PyEntityCommands is Send because:
// - Entity is Copy + Send
// - The raw pointer is stored as usize (just an address)
// - Access through get_commands() requires the PyCommands instance to still be valid
unsafe impl Send for PyEntityCommands {}

// SAFETY: PyEntityCommands is Sync because:
// - All fields are either Copy or contain addresses
// - Actual access to commands is controlled by the PyCommands validity checking
unsafe impl Sync for PyEntityCommands {}

impl PyEntityCommands {
    pub(crate) fn with_commands(entity: Entity, commands: &PyCommands) -> Self {
        Self {
            id: entity,
            commands_ptr: Some(commands as *const PyCommands as usize),
            world_ptr: None,
            validity: Some(commands.validity()),
        }
    }

    pub(crate) fn with_world(entity: Entity, world: &super::world::PyWorld) -> Self {
        Self {
            id: entity,
            commands_ptr: None,
            world_ptr: Some(world as *const super::world::PyWorld as usize),
            validity: world.validity(),
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
        Ok(self
            .commands_ptr
            .map(|ptr| unsafe { &*(ptr as *const PyCommands) }))
    }

    fn get_world(&self) -> PyResult<Option<&super::world::PyWorld>> {
        self.check_valid()?;
        Ok(self
            .world_ptr
            .map(|ptr| unsafe { &*(ptr as *const super::world::PyWorld) }))
    }

    /// Create temporary PyCommands from the world pointer for entity operations.
    /// Returns None if no world pointer is available.
    fn temp_commands_from_world(&self) -> PyResult<Option<PyCommands>> {
        if let Some(world) = self.get_world()? {
            let world_ptr = world.world_ptr();
            let validity = world.validity().unwrap_or_else(ValidityFlag::new);
            // SAFETY: We're creating a temporary PyCommands that wraps the World pointer.
            // The world pointer is valid because we just checked validity via get_world().
            let temp_commands = unsafe { PyCommands::from_world_temporary(world_ptr, validity) };
            Ok(Some(temp_commands))
        } else {
            Ok(None)
        }
    }

    /// Get a PyCommands reference, either from stored commands or by creating
    /// temporary commands from the world pointer. Returns the commands and
    /// whether they are temporary (and thus must not be referenced after this call).
    fn get_commands_or_world(&self) -> PyResult<Option<CommandsSource<'_>>> {
        if let Some(commands) = self.get_commands()? {
            Ok(Some(CommandsSource::Commands(commands)))
        } else if let Some(temp) = self.temp_commands_from_world()? {
            Ok(Some(CommandsSource::TempFromWorld(temp)))
        } else {
            Ok(None)
        }
    }
}

/// Either a borrowed reference to stored PyCommands or a temporary one created from World.
enum CommandsSource<'a> {
    Commands(&'a PyCommands),
    TempFromWorld(PyCommands),
}

impl<'a> CommandsSource<'a> {
    fn as_ref(&self) -> &PyCommands {
        match self {
            CommandsSource::Commands(c) => c,
            CommandsSource::TempFromWorld(c) => c,
        }
    }
}

#[pymethods]
impl PyEntityCommands {
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
        if let Some(source) = self.get_commands_or_world()? {
            crate::ecs::commands::insert_components_to_entity_helper(
                source.as_ref(),
                py,
                self.id,
                components,
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
        if let Some(source) = self.get_commands_or_world()? {
            crate::ecs::commands::remove_components_from_entity_helper(
                source.as_ref(),
                py,
                self.id,
                components,
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
        if let Some(source) = self.get_commands_or_world()? {
            crate::ecs::commands::trigger_event_helper(source.as_ref(), py, event, Some(self.id))?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot trigger event: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Despawn this entity
    pub fn despawn(&self) -> PyResult<()> {
        if let Some(source) = self.get_commands_or_world()? {
            source.as_ref().despawn(&PyEntity(self.id))
        } else {
            Err(PyValueError::new_err(
                "Cannot despawn: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Add a child entity to this entity
    pub fn add_child(&self, child: &PyEntity) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands_or_world()? {
            crate::ecs::commands::add_child_helper(source.as_ref(), self.id, child.0)?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot add child: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Set the parent of this entity
    pub fn set_parent(&self, parent: &PyEntity) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands_or_world()? {
            crate::ecs::commands::set_parent_helper(source.as_ref(), self.id, parent.0)?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot set parent: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Remove the parent relationship from this entity
    pub fn remove_parent(&self) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands_or_world()? {
            crate::ecs::commands::remove_parent_helper(source.as_ref(), self.id)?;
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
        if let Some(source) = self.get_commands_or_world()? {
            let child_ids: Vec<Entity> = children
                .iter()
                .map(|item| {
                    item.extract::<PyEntity>()
                        .map(|e| e.0)
                        .map_err(|_| PyTypeError::new_err("Expected Entity objects"))
                })
                .collect::<PyResult<Vec<_>>>()?;

            crate::ecs::commands::remove_children_helper(source.as_ref(), self.id, &child_ids)?;
            Ok(self.clone())
        } else {
            Err(PyValueError::new_err(
                "Cannot remove children: EntityCommands not associated with a Commands or World object.",
            ))
        }
    }

    /// Remove all children from this entity
    pub fn clear_children(&self) -> PyResult<PyEntityCommands> {
        if let Some(source) = self.get_commands_or_world()? {
            crate::ecs::commands::clear_children_helper(source.as_ref(), self.id)?;
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

        // Anchor the spawner to whichever backing this handle owns. Routing a
        // World-backed handle through a temporary PyCommands would hand the
        // spawner a pointer to a stack local fenced by the World's much
        // longer-lived validity flag.
        let spawner = if let Some(commands) = self.get_commands()? {
            PyRelatedSpawnerCommands::with_commands(self.id, commands)
        } else if let Some(world) = self.get_world()? {
            PyRelatedSpawnerCommands::with_world(self.id, world)
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
        let world_mut = if let Some(commands) = self.get_commands()? {
            // Via Commands (immediate mode only)
            commands.try_world_mut()?
        } else if let Some(world) = self.get_world()? {
            // Via World (direct access)
            Some(world.world_mut()?)
        } else {
            None
        };

        if let Some(world) = world_mut {
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

/// Helper for spawning entities that are related to a target entity (e.g., children)
///
/// Mirrors [`PyEntityCommands`]: exactly one of `commands_ptr` / `world_ptr` is
/// set, and both address a live Python object whose lifetime the shared
/// `validity` flag fences. A World-backed spawner rebuilds its temporary
/// `PyCommands` per operation rather than storing a pointer to one.
#[pyclass(name = "RelatedSpawnerCommands", module = "pybevy.ecs")]
pub struct PyRelatedSpawnerCommands {
    target: Entity,
    commands_ptr: Option<usize>,
    world_ptr: Option<usize>,
    // Runtime validity check - prevents use after system execution
    validity: ValidityFlag,
}

// SAFETY: PyRelatedSpawnerCommands is Send because:
// - Entity is Copy + Send
// - The raw pointer is stored as usize (just an address)
// - ValidityFlag is Arc<AtomicBool> which is Send + Sync
// - Access through get_commands() requires validity check
unsafe impl Send for PyRelatedSpawnerCommands {}

// SAFETY: PyRelatedSpawnerCommands is Sync because:
// - All fields are either Copy or thread-safe (ValidityFlag)
// - Actual access to commands is controlled by validity checking
unsafe impl Sync for PyRelatedSpawnerCommands {}

impl PyRelatedSpawnerCommands {
    fn with_commands(target: Entity, commands: &PyCommands) -> Self {
        Self {
            target,
            commands_ptr: Some(commands as *const PyCommands as usize),
            world_ptr: None,
            validity: commands.validity(),
        }
    }

    fn with_world(target: Entity, world: &super::world::PyWorld) -> Self {
        Self {
            target,
            commands_ptr: None,
            world_ptr: Some(world as *const super::world::PyWorld as usize),
            validity: world.validity().unwrap_or_default(),
        }
    }

    fn commands_source(&self) -> PyResult<CommandsSource<'_>> {
        self.validity.check()?;
        if let Some(ptr) = self.commands_ptr {
            return Ok(CommandsSource::Commands(unsafe {
                &*(ptr as *const PyCommands)
            }));
        }
        let Some(ptr) = self.world_ptr else {
            return Err(PyValueError::new_err(
                "RelatedSpawnerCommands not properly initialized",
            ));
        };
        let world = unsafe { &*(ptr as *const super::world::PyWorld) };
        let world_ptr = world.world_ptr();
        let validity = world.validity().unwrap_or_else(ValidityFlag::new);
        // SAFETY: the World pointer stays valid while `validity` is active, and
        // the temporary never outlives this call.
        let temp = unsafe { PyCommands::from_world_temporary(world_ptr, validity) };
        Ok(CommandsSource::TempFromWorld(temp))
    }

    /// Re-anchor a handle produced by a temporary adapter back onto this
    /// spawner's own backing, so it never retains the temporary's address.
    fn reanchor(&self, entity_cmd: &mut PyEntityCommands) {
        entity_cmd.commands_ptr = self.commands_ptr;
        entity_cmd.world_ptr = self.world_ptr;
        entity_cmd.validity = Some(self.validity.clone());
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
        let commands_ptr = &*commands_ref as *const PyCommands as usize;
        let validity = commands_ref.validity();
        Ok(Self {
            target: target.0,
            commands_ptr: Some(commands_ptr),
            world_ptr: None,
            validity,
        })
    }

    /// Spawn an empty entity as a child
    pub fn spawn_empty(&self, py: Python) -> PyResult<PyEntityCommands> {
        let source = self.commands_source()?;
        let mut entity_cmd = source.as_ref().spawn_empty(py)?;

        // Insert ChildOf component to establish parent-child relationship
        let child_of = Self::create_child_of_component(py, self.target)?;
        let child_of_tuple = PyTuple::new(py, vec![child_of])?;
        entity_cmd.insert(py, &child_of_tuple)?;

        self.reanchor(&mut entity_cmd);
        Ok(entity_cmd)
    }

    /// Spawn an entity with components as a child
    #[pyo3(signature = (*components))]
    pub fn spawn(&self, py: Python, components: &Bound<'_, PyTuple>) -> PyResult<PyEntityCommands> {
        let source = self.commands_source()?;
        let mut entity_cmd = source.as_ref().spawn(py, components)?;

        // Insert ChildOf component to establish parent-child relationship
        let child_of = Self::create_child_of_component(py, self.target)?;
        let child_of_tuple = PyTuple::new(py, vec![child_of])?;
        entity_cmd.insert(py, &child_of_tuple)?;

        self.reanchor(&mut entity_cmd);
        Ok(entity_cmd)
    }

    /// Get the target entity ID
    pub fn target_entity(&self) -> PyEntity {
        PyEntity(self.target)
    }
}
