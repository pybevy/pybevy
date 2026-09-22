use bevy::ecs::{entity::Entity, hierarchy::ChildOf, name::Name, world::World};
use pybevy_core::public_error;

use crate::bridge::{ControlError, EntityRef};

fn ambiguous_name_error(name: &str, matches: &[Entity]) -> String {
    let count = matches.len();
    let mut ids = matches
        .iter()
        .take(5)
        .map(|entity| entity.to_bits().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if count > 5 {
        ids.push_str(&format!(" ... ({} more)", count - 5));
    }
    format!(
        "Entity name '{name}' is ambiguous: {count} matches (IDs: {ids}). Disambiguate by passing the entity_id."
    )
}

/// Resolve an MCP entity reference without depending on an interpreter backend.
///
/// A unique root match takes precedence over child matches for compatibility
/// with hierarchy-aware scene tools. Multiple matches at the selected
/// precedence are rejected instead of silently mutating an arbitrary entity.
pub fn resolve_entity(world: &mut World, entity_ref: &EntityRef) -> Result<Entity, ControlError> {
    match entity_ref {
        EntityRef::Id(id) => {
            let entity = Entity::try_from_bits(*id)
                .ok_or_else(|| ControlError::not_found(public_error::mcp_entity_not_found(*id)))?;
            world
                .get_entity(entity)
                .map(|_| entity)
                .map_err(|_| ControlError::not_found(public_error::mcp_entity_not_found(*id)))
        }
        EntityRef::Name(name) => {
            let mut roots = Vec::new();
            let mut children = Vec::new();
            let mut query = world.query::<(Entity, &Name)>();
            for (entity, entity_name) in query.iter(world) {
                if entity_name.as_str() != name {
                    continue;
                }
                if world.get::<ChildOf>(entity).is_none() {
                    roots.push(entity);
                } else {
                    children.push(entity);
                }
            }

            let candidates = if roots.is_empty() { &children } else { &roots };
            match candidates.as_slice() {
                [] => Err(ControlError::not_found(format!(
                    "Entity with name '{name}' not found"
                ))),
                [entity] => Ok(*entity),
                _ => Err(ControlError::invalid_params(ambiguous_name_error(
                    name, candidates,
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::name::Name;

    use super::*;
    use crate::bridge::ErrorCode;

    #[test]
    fn duplicate_roots_are_ambiguous() {
        let mut world = World::new();
        let a = world.spawn(Name::new("lantern")).id();
        let b = world.spawn(Name::new("lantern")).id();
        let error = resolve_entity(&mut world, &EntityRef::Name("lantern".into())).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(error.message.contains("2 matches"));
        assert!(error.message.contains(&a.to_bits().to_string()));
        assert!(error.message.contains(&b.to_bits().to_string()));
    }

    #[test]
    fn unique_root_wins_over_children() {
        let mut world = World::new();
        let parent = world.spawn(Name::new("Parent")).id();
        world.spawn((Name::new("Beacon"), ChildOf(parent)));
        let root = world.spawn(Name::new("Beacon")).id();
        assert_eq!(
            resolve_entity(&mut world, &EntityRef::Name("Beacon".into())).unwrap(),
            root
        );
    }

    #[test]
    fn numeric_not_found_names_packed_identity_and_python_conversions() {
        let mut world = World::new();
        let error = resolve_entity(&mut world, &EntityRef::Id(42)).unwrap_err();
        assert_eq!(
            error.message,
            "Entity 42 not found. Numeric MCP/HTTP entity IDs are packed Entity.to_bits() values, not Entity.index() or the index shown in Entity(<index>v<generation>). Use an ID returned by query_entities or another MCP response; in Python, convert with Entity.from_bits(id) and produce one with entity.to_bits()."
        );
    }

    #[test]
    fn stale_generation_cannot_resolve_to_reused_index() {
        let mut world = World::new();
        let stale_entities = (0..129)
            .map(|_| world.spawn_empty().id())
            .collect::<Vec<_>>();
        for stale in &stale_entities {
            world.despawn(*stale);
        }
        let replacement = world.spawn_empty().id();
        let stale = stale_entities
            .into_iter()
            .find(|entity| entity.index() == replacement.index())
            .unwrap();

        assert_ne!(replacement.to_bits(), stale.to_bits());
        let error = resolve_entity(&mut world, &EntityRef::Id(stale.to_bits())).unwrap_err();
        assert!(
            error
                .message
                .starts_with(&format!("Entity {} not found", stale.to_bits()))
        );
        assert_eq!(
            resolve_entity(&mut world, &EntityRef::Id(replacement.to_bits())).unwrap(),
            replacement
        );
    }
}
