use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use bevy::ecs::{entity::Entity, resource::Resource, world::World};
use pybevy_core::{
    CustomComponentInfo, CustomResourceInfo,
    public_error::{WORLD_COMMAND_ERRORS, WORLD_OPERATION_COMMAND_ERRORS},
};
use pyo3::{PyTraverseError, PyVisit, exceptions::PyBaseException, prelude::*, types::PyTuple};

use super::{
    PyEntity,
    batch_spawn::SpawnBatchCommand,
    commands::{self, PreparedInsertion, PyCommands},
    component_type::ComponentRegistry,
    dynamic_system::lock_or_recover,
    helpers::validity_guard::{ValidityFlag, ValidityGuard},
    observer_registry::ObserverRegistry,
    resource_type::{PyResourceType, ResourceRegistry},
};

pub(crate) enum WorldCommand {
    Mutation(WorldMutation),
    Insert(Entity, Box<PreparedInsertion>),
    Batch(Vec<(SpawnBatchCommand, Vec<Entity>)>),
    Remove(Entity, Py<PyTuple>),
    InsertResource(Py<PyAny>),
    RemoveResource(Py<PyAny>),
    Trigger(Option<Entity>, Py<PyAny>),
    Observe(Entity, Py<PyAny>),
}

pub(crate) enum WorldMutation {
    Despawn(Entity),
    AddChild(Entity, Entity),
    RemoveChildren(Entity, Vec<Entity>),
    ClearChildren(Entity),
    SetParent(Entity, Entity),
    RemoveParent(Entity),
}

impl WorldCommand {
    fn traverse(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        match self {
            Self::Mutation(_) => Ok(()),
            Self::Insert(_, insertion) => insertion.traverse(visit),
            Self::Batch(batches) => {
                for (batch, _) in batches {
                    batch.traverse(visit.clone())?;
                }
                Ok(())
            }
            Self::Remove(_, values) => visit.call(values),
            Self::InsertResource(value)
            | Self::RemoveResource(value)
            | Self::Trigger(_, value)
            | Self::Observe(_, value) => visit.call(value),
        }
    }

    fn apply(self, world: &mut World, py: Python<'_>) -> PyResult<()> {
        let validity = ValidityFlag::new();
        let _guard = ValidityGuard::for_world(validity.clone(), world.id());
        match self {
            Self::Mutation(mutation) => {
                // SAFETY: this adapter cannot escape exclusive command application.
                let adapter = unsafe { PyCommands::from_world_temporary(world, validity) };
                match mutation {
                    WorldMutation::Despawn(entity) => adapter.despawn_entity(&PyEntity(entity)),
                    WorldMutation::AddChild(parent, child) => {
                        commands::add_child_helper(&adapter, parent, child)
                    }
                    WorldMutation::RemoveChildren(parent, children) => {
                        commands::remove_children_helper(&adapter, parent, &children)
                    }
                    WorldMutation::ClearChildren(parent) => {
                        commands::clear_children_helper(&adapter, parent)
                    }
                    WorldMutation::SetParent(child, parent) => {
                        commands::set_parent_helper(&adapter, child, parent)
                    }
                    WorldMutation::RemoveParent(child) => {
                        commands::remove_parent_helper(&adapter, child)
                    }
                }
            }
            Self::Insert(entity, insertion) => insertion.apply(world, entity),
            Self::Batch(batches) => {
                for (batch, entities) in batches {
                    batch.apply_to(world, entities)?;
                }
                Ok(())
            }
            Self::InsertResource(value) => {
                let kind = PyResourceType::try_from((&value.bind(py).get_type(), py))?;
                kind.insert_into_world(world, py, value)
            }
            Self::RemoveResource(class) => {
                let kind = PyResourceType::try_from((class.bind(py).cast()?, py))?;
                kind.remove_from_world(world, py)
            }
            Self::Observe(entity, observer) => {
                ObserverRegistry::register_observer_for_entity(
                    py,
                    observer.bind(py),
                    entity,
                    world,
                )?;
                Ok(())
            }
            Self::Remove(entity, classes) => {
                // SAFETY: the temporary adapter is confined to this exclusive command application.
                let commands = unsafe { PyCommands::from_world_temporary(world, validity) };
                super::commands::remove_components_from_entity_helper(
                    &commands,
                    py,
                    entity,
                    classes.bind(py),
                )
            }
            Self::Trigger(target, event) => {
                // SAFETY: the temporary adapter is confined to this exclusive command application.
                let commands = unsafe { PyCommands::from_world_temporary(world, validity) };
                super::commands::trigger_event_helper(&commands, py, event.into_bound(py), target)
            }
        }
    }
}

#[derive(Default)]
struct PendingCommands {
    next: u64,
    operations: HashMap<u64, WorldCommand>,
}

#[derive(Resource, Default)]
pub(crate) struct WorldCommandState {
    pending: Arc<Mutex<PendingCommands>>,
    pub(crate) errors: Arc<Mutex<Vec<Py<PyBaseException>>>>,
}

#[derive(Default)]
pub(crate) struct ErrorBoundary {
    errors: Option<Weak<Mutex<Vec<Py<PyBaseException>>>>>,
    start: usize,
}

impl ErrorBoundary {
    pub(crate) fn new(world: &World) -> Self {
        let Some(state) = world.get_resource::<WorldCommandState>() else {
            return Self::default();
        };
        Self {
            errors: Some(Arc::downgrade(&state.errors)),
            start: lock_or_recover(&state.errors).len(),
        }
    }

    pub(crate) fn run<T>(self, operation: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
        let result = operation();
        Python::attach(|py| {
            let mut errors = self
                .errors
                .and_then(|errors| errors.upgrade())
                .map(|errors| {
                    let values = {
                        let mut errors = lock_or_recover(&errors);
                        let start = self.start.min(errors.len());
                        errors.split_off(start)
                    };
                    values
                        .into_iter()
                        .map(|value| PyErr::from_value(value.into_bound(py).into_any()))
                        .collect()
                })
                .unwrap_or_default();
            match result {
                Ok(value) => raise_collected(py, errors, WORLD_COMMAND_ERRORS).map(|()| value),
                Err(error) => {
                    errors.insert(0, error);
                    raise_collected(py, errors, WORLD_OPERATION_COMMAND_ERRORS)?;
                    unreachable!("the operation failure is always present")
                }
            }
        })
    }
}

pub(crate) fn initialize(world: &mut World) {
    // First-time query registration must not flush commands while creating metadata resources.
    world.init_resource::<WorldCommandState>();
    world.init_resource::<ComponentRegistry>();
    world.init_resource::<CustomComponentInfo>();
    world.init_resource::<ResourceRegistry>();
    world.init_resource::<CustomResourceInfo>();
}

impl WorldCommandState {
    pub(crate) fn traverse(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        let Ok(pending) = self.pending.try_lock() else {
            return Ok(());
        };
        for operation in pending.operations.values() {
            operation.traverse(visit.clone())?;
        }
        let Ok(errors) = self.errors.try_lock() else {
            return Ok(());
        };
        for error in errors.iter() {
            visit.call(error)?;
        }
        Ok(())
    }
}

pub(crate) fn enqueue(world: &mut World, command: WorldCommand) {
    let state = world.get_resource_or_insert_with(WorldCommandState::default);
    let pending = Arc::downgrade(&state.pending);
    let errors = Arc::downgrade(&state.errors);
    let id = {
        let mut pending = lock_or_recover(&state.pending);
        let id = pending.next;
        pending.next = pending
            .next
            .checked_add(1)
            .expect("World command identity exhausted");
        pending.operations.insert(id, command);
        id
    };
    world.commands().queue(move |world: &mut World| {
        Python::attach(|py| {
            let Some(pending) = pending.upgrade() else {
                return;
            };
            let command = lock_or_recover(&pending).operations.remove(&id);
            if let Some(command) = command
                && let Err(error) = command.apply(world, py)
                && let Some(errors) = errors.upgrade()
            {
                let value = error.into_value(py);
                lock_or_recover(&errors).push(value);
            }
        });
    });
}

pub(crate) fn take_errors(world: &World, py: Python<'_>) -> Vec<PyErr> {
    let Some(state) = world.get_resource::<WorldCommandState>() else {
        return Vec::new();
    };
    take_stored_errors(&state.errors, py)
}

fn take_stored_errors(errors: &Mutex<Vec<Py<PyBaseException>>>, py: Python<'_>) -> Vec<PyErr> {
    let errors = std::mem::take(&mut *lock_or_recover(errors));
    errors
        .into_iter()
        .map(|error| PyErr::from_value(error.into_bound(py).into_any()))
        .collect()
}

pub(crate) fn raise_errors(world: &World, py: Python<'_>) -> PyResult<()> {
    raise_collected(py, take_errors(world, py), WORLD_COMMAND_ERRORS)
}

pub(crate) fn raise_collected(
    py: Python<'_>,
    mut errors: Vec<PyErr>,
    message: &str,
) -> PyResult<()> {
    match errors.len() {
        0 => Ok(()),
        1 => Err(errors.pop().unwrap()),
        _ => {
            let values = errors
                .into_iter()
                .map(|error| error.into_value(py))
                .collect::<Vec<_>>();
            let group = py
                .import("builtins")?
                .getattr("BaseExceptionGroup")?
                .call1((message, values))?;
            Err(PyErr::from_value(group))
        }
    }
}
