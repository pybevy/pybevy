//! Interpreter-neutral live [`World`] extraction and serialization.

use std::{collections::HashSet, error::Error, fmt};

use bevy::{ecs::reflect::AppTypeRegistry, prelude::World, world_serialization::DynamicWorld};
use pybevy_core::{
    component_layout::ComponentStorageType,
    custom_component::CustomComponentRegistry,
    custom_resource::CustomResourceRegistry,
    public_error::{
        WORLD_SERIALIZATION_TYPE_REGISTRY_MISSING, world_serialization_failed,
        world_serialization_skipped_custom_types,
    },
};

use crate::custom_component::{extract_custom_components, register_custom_component_reflection};

/// Failure produced while extracting or serializing a live World.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveWorldSerializationError {
    /// The World does not contain Bevy's application type registry.
    MissingTypeRegistry,
    /// Bevy's RON serializer rejected the extracted data.
    Serialization(String),
    /// A registered wrapper component could not be read safely.
    WrapperComponent(String),
}

impl fmt::Display for LiveWorldSerializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTypeRegistry => {
                formatter.write_str(WORLD_SERIALIZATION_TYPE_REGISTRY_MISSING)
            }
            Self::Serialization(error) => formatter.write_str(&world_serialization_failed(error)),
            Self::WrapperComponent(error) => formatter.write_str(error),
        }
    }
}

impl Error for LiveWorldSerializationError {}

/// Custom Python ECS types omitted from a reflected DynamicWorld snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkippedCustomTypes {
    pub components: Vec<String>,
    pub resources: Vec<String>,
}

impl SkippedCustomTypes {
    pub fn is_empty(&self) -> bool {
        self.components.is_empty() && self.resources.is_empty()
    }

    pub fn warning_message(&self) -> String {
        world_serialization_skipped_custom_types(&self.components, &self.resources)
    }
}

/// Owned snapshot plus diagnostics about custom values Bevy could not extract.
pub struct LiveWorldExtraction {
    pub dynamic_world: DynamicWorld,
    pub skipped_custom_types: SkippedCustomTypes,
}

fn type_registry(world: &World) -> Result<AppTypeRegistry, LiveWorldSerializationError> {
    let registry = world
        .get_resource::<AppTypeRegistry>()
        .cloned()
        .ok_or(LiveWorldSerializationError::MissingTypeRegistry)?;
    register_custom_component_reflection(&mut registry.write());
    Ok(registry)
}

fn extract_with_registry(
    world: &World,
    registry: &AppTypeRegistry,
) -> Result<DynamicWorld, LiveWorldSerializationError> {
    let mut dynamic_world = DynamicWorld::from_world_with(world, &registry.read());
    for dynamic_entity in &mut dynamic_world.entities {
        if let Some(custom_components) = extract_custom_components(world, dynamic_entity.entity)
            .map_err(LiveWorldSerializationError::WrapperComponent)?
        {
            dynamic_entity.components.push(Box::new(custom_components));
        }
    }
    Ok(dynamic_world)
}

fn serialize_with_registry(
    dynamic_world: &DynamicWorld,
    registry: &AppTypeRegistry,
) -> Result<String, LiveWorldSerializationError> {
    dynamic_world
        .serialize(&registry.read())
        .map_err(|error| LiveWorldSerializationError::Serialization(error.to_string()))
}

fn skipped_custom_types(world: &World) -> SkippedCustomTypes {
    let present_component_ids = world
        .archetypes()
        .iter()
        .filter(|archetype| !archetype.is_empty())
        .flat_map(|archetype| archetype.components().iter().copied())
        .collect::<HashSet<_>>();
    let mut components = world
        .get_resource::<CustomComponentRegistry>()
        .into_iter()
        .flat_map(CustomComponentRegistry::ids_by_qualified_name)
        .filter(|(_, component_id)| present_component_ids.contains(component_id))
        .filter(|(_, component_id)| {
            world
                .resource::<CustomComponentRegistry>()
                .storage_type(*component_id)
                == Some(ComponentStorageType::PyObject)
        })
        .map(|(name, _)| name.to_owned())
        .collect::<Vec<_>>();
    let mut resources = world
        .get_resource::<CustomResourceRegistry>()
        .into_iter()
        .flat_map(CustomResourceRegistry::ids_by_qualified_name)
        .filter(|(_, component_id)| {
            world
                .resource_entities()
                .get(*component_id)
                .and_then(|entity| world.get_entity(entity).ok())
                .is_some_and(|entity| entity.contains_id(*component_id))
        })
        .map(|(name, _)| name.to_owned())
        .collect::<Vec<_>>();
    components.sort();
    resources.sort();
    SkippedCustomTypes {
        components,
        resources,
    }
}

/// Extract all reflectable data from a live World into an owned DynamicWorld.
pub fn extract_live_world(
    world: &World,
) -> Result<LiveWorldExtraction, LiveWorldSerializationError> {
    let registry = type_registry(world)?;
    Ok(LiveWorldExtraction {
        dynamic_world: extract_with_registry(world, &registry)?,
        skipped_custom_types: skipped_custom_types(world),
    })
}

/// Serialize a DynamicWorld with the registry from a live World.
pub fn serialize_dynamic_world(
    dynamic_world: &DynamicWorld,
    world: &World,
) -> Result<String, LiveWorldSerializationError> {
    let registry = type_registry(world)?;
    serialize_with_registry(dynamic_world, &registry)
}
