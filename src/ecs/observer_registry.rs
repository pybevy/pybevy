use std::collections::HashMap;

use bevy::{ecs::world::World, prelude::Resource};
use pyo3::prelude::*;

use super::{observer::EventType, system::SystemFunction};

/// Observer entry storing the function and its event type
#[derive(Debug, Clone)]
pub struct ObserverEntry {
    /// The observer entity (can be used to despawn the observer)
    pub(crate) observer_entity: bevy::ecs::entity::Entity,
    /// The system function (with parsed parameters)
    pub(crate) system_func: SystemFunction,
    /// Optional bundle filter (observer only triggers if entity has these components)
    pub(crate) bundle_filter: Option<Vec<crate::ecs::component_type::PyComponentType>>,
    /// Optional entity filter (observer only triggers for this specific entity)
    pub(crate) entity_filter: Option<bevy::ecs::entity::Entity>,
}

/// Registry for observer functions
/// Maps EventType -> Vec<ObserverEntry>
#[derive(Debug, Default, Resource)]
pub struct ObserverRegistry {
    /// Map from event type to list of observer functions
    pub(crate) observers: HashMap<String, Vec<ObserverEntry>>,
}

impl ObserverRegistry {
    /// Register an observer function for an event type
    /// Returns the observer entity ID which can be used to despawn the observer
    pub fn register_observer(
        py: Python,
        func: &Bound<'_, PyAny>,
        world: &mut World,
    ) -> PyResult<bevy::ecs::entity::Entity> {
        // Parse the system function to extract parameters
        let system_func = SystemFunction::new(py, func.clone())?;

        // Find the On parameter to extract event type
        let (event_type, bundle_filter) = Self::extract_event_type_from_params(&system_func)?;

        // Spawn an entity to represent this observer
        let observer_entity = world.spawn_empty().id();

        // Create observer entry (global observer - no entity filter)
        let observer_entry = ObserverEntry {
            observer_entity,
            system_func,
            bundle_filter,
            entity_filter: None,
        };

        // Get or create the registry resource
        if !world.contains_resource::<ObserverRegistry>() {
            world.insert_resource(ObserverRegistry::default());
        }

        // Get the event type key (use Debug format for now)
        let event_key = format!("{:?}", event_type);

        // Add the observer to the registry
        let mut registry = world.resource_mut::<ObserverRegistry>();
        registry
            .observers
            .entry(event_key)
            .or_default()
            .push(observer_entry);

        Ok(observer_entity)
    }

    /// Register an observer function for a specific entity
    /// Returns the observer entity ID which can be used to despawn the observer
    pub fn register_observer_for_entity(
        py: Python,
        func: &Bound<'_, PyAny>,
        entity: bevy::ecs::entity::Entity,
        world: &mut World,
    ) -> PyResult<bevy::ecs::entity::Entity> {
        // Parse the system function to extract parameters
        let system_func = SystemFunction::new(py, func.clone())?;

        // Find the On parameter to extract event type
        let (event_type, bundle_filter) = Self::extract_event_type_from_params(&system_func)?;

        // Spawn an entity to represent this observer
        let observer_entity = world.spawn_empty().id();

        // Create observer entry (entity-specific observer)
        let observer_entry = ObserverEntry {
            observer_entity,
            system_func,
            bundle_filter,
            entity_filter: Some(entity),
        };

        // Get or create the registry resource
        if !world.contains_resource::<ObserverRegistry>() {
            world.insert_resource(ObserverRegistry::default());
        }

        // Get the event type key
        let event_key = format!("{:?}", event_type);

        // Add the observer to the registry
        let mut registry = world.resource_mut::<ObserverRegistry>();
        registry
            .observers
            .entry(event_key)
            .or_default()
            .push(observer_entry);

        Ok(observer_entity)
    }

    /// Extract the event type from the On parameter in the system function
    fn extract_event_type_from_params(
        system_func: &SystemFunction,
    ) -> PyResult<(
        EventType,
        Option<Vec<crate::ecs::component_type::PyComponentType>>,
    )> {
        use crate::ecs::system::SystemParamType;

        // Find the On parameter
        for param in &system_func.params {
            if let SystemParamType::On {
                event_type,
                bundle_filter,
            } = &param.ty
            {
                return Ok((event_type.clone(), bundle_filter.clone()));
            }
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "Observer function must have an On[EventType] parameter",
        ))
    }

    /// Remove all registered observers and return their entity IDs for despawning.
    /// Used during hot reload to clean up before re-registration.
    pub(crate) fn clear_all(&mut self) -> Vec<bevy::ecs::entity::Entity> {
        let mut entities = Vec::new();
        for entries in self.observers.values() {
            for entry in entries {
                entities.push(entry.observer_entity);
            }
        }
        self.observers.clear();
        entities
    }

    /// Get all observers for a given event type
    pub fn get_observers(&self, event_type: &EventType) -> Option<&Vec<ObserverEntry>> {
        let event_key = format!("{:?}", event_type);
        self.observers.get(&event_key)
    }

    /// Get all observers for a Python event instance
    pub fn get_observers_for_event(
        &self,
        _py: Python,
        event: &Bound<'_, PyAny>,
    ) -> PyResult<Option<&Vec<ObserverEntry>>> {
        // Get the event's Python type
        let event_py_type = event.get_type();

        // Create an EventType::Custom from it
        let event_type = EventType::Custom(event_py_type.unbind());

        Ok(self.get_observers(&event_type))
    }

    /// Remove an observer from the registry by its entity ID
    /// Returns true if the observer was found and removed
    pub fn remove_observer(&mut self, observer_entity: bevy::ecs::entity::Entity) -> bool {
        // Search through all event types to find and remove the observer
        for observers in self.observers.values_mut() {
            if let Some(index) = observers
                .iter()
                .position(|entry| entry.observer_entity == observer_entity)
            {
                observers.remove(index);
                return true;
            }
        }
        false
    }

    /// Despawn an observer entity and remove it from the registry
    pub fn despawn_observer(
        observer_entity: bevy::ecs::entity::Entity,
        world: &mut World,
    ) -> PyResult<()> {
        // Remove from registry
        if let Some(mut registry) = world.get_resource_mut::<ObserverRegistry>() {
            registry.remove_observer(observer_entity);
        }

        // Despawn the entity
        if let Ok(entity_mut) = world.get_entity_mut(observer_entity) {
            entity_mut.despawn();
        }

        Ok(())
    }

    /// Remove all observers that are watching a specific entity
    /// This is called when an entity is despawned to clean up per-entity observers
    pub fn cleanup_observers_for_entity(
        &mut self,
        watched_entity: bevy::ecs::entity::Entity,
    ) -> Vec<bevy::ecs::entity::Entity> {
        let mut removed_observers = Vec::new();

        // Search through all event types and remove observers watching this entity
        for observers in self.observers.values_mut() {
            let mut i = 0;
            while i < observers.len() {
                if let Some(filter_entity) = observers[i].entity_filter
                    && filter_entity == watched_entity
                {
                    // This observer was watching the despawned entity
                    let removed = observers.remove(i);
                    removed_observers.push(removed.observer_entity);
                    continue; // Don't increment i, check same index again
                }
                i += 1;
            }
        }

        removed_observers
    }

    /// Clean up per-entity observers when an entity is despawned
    /// This removes observers from the registry and despawns their entities
    pub fn cleanup_on_entity_despawn(watched_entity: bevy::ecs::entity::Entity, world: &mut World) {
        // Get the list of observer entities to despawn
        let observer_entities =
            if let Some(mut registry) = world.get_resource_mut::<ObserverRegistry>() {
                registry.cleanup_observers_for_entity(watched_entity)
            } else {
                return; // No registry, nothing to clean up
            };

        // Despawn the observer entities
        for observer_entity in observer_entities {
            if let Ok(entity_mut) = world.get_entity_mut(observer_entity) {
                entity_mut.despawn();
            }
        }
    }
}
