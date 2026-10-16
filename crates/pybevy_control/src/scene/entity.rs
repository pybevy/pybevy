use std::sync::OnceLock;

use bevy::ecs::{
    entity::Entity, entity_disabling::Disabled, hierarchy::ChildOf, name::Name, query::Allow,
    world::World,
};
use pybevy_core::{custom_resource::hierarchy_contains_resource_entity, public_error};

use crate::protocol::{ControlError, EntityRef};

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
            let mut query = world.query_filtered::<(Entity, &Name), Allow<Disabled>>();
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

type DespawnEntityHook = fn(&mut World, Entity);

static DESPAWN_ENTITY_HOOK: OnceLock<DespawnEntityHook> = OnceLock::new();

/// Lets a backend despawn through its own lifecycle path so Python
/// `on_remove` and `on_despawn` observers run.
pub fn register_despawn_entity_hook(hook: DespawnEntityHook) {
    let _ = DESPAWN_ENTITY_HOOK.set(hook);
}

/// Check if a tool result contains embedded field-level errors.
/// Used by batch and schedule to detect partial failures.
pub fn has_embedded_errors(value: &serde_json::Value) -> bool {
    value
        .get("errors")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| !arr.is_empty())
}

/// Despawn an entity by ID or Name
pub fn despawn_entity(
    world: &mut World,
    entity_ref: EntityRef,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();

    // Bevy cascades the despawn through descendants, so a resource entity
    // anywhere in the subtree would have its value silently discarded.
    if hierarchy_contains_resource_entity(world, entity) {
        return Err(ControlError::invalid_params(
            public_error::RESOURCE_ENTITY_DESPAWN,
        ));
    }

    let existed = world.entities().contains(entity);
    if existed {
        match DESPAWN_ENTITY_HOOK.get() {
            Some(hook) => hook(world, entity),
            None => {
                world.despawn(entity);
            }
        }
    }

    if existed {
        Ok(serde_json::json!({
            "despawned": true,
            "entity_id": entity_id,
        }))
    } else {
        Err(ControlError::not_found(public_error::mcp_entity_not_found(
            entity_id,
        )))
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        camera::ClearColor,
        ecs::{hierarchy::Children, name::Name, query::With, resource::IsResource},
    };

    use super::*;
    use crate::protocol::ErrorCode;

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
    fn disabled_entity_resolves_by_name() {
        let mut world = World::new();
        world.register_disabling_component::<Disabled>();
        let disabled = world.spawn((Name::new("Hidden"), Disabled)).id();

        assert_eq!(
            resolve_entity(&mut world, &EntityRef::Name("Hidden".into())).unwrap(),
            disabled
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

    #[test]
    fn resolve_entity_prefers_root_over_child() {
        let mut world = World::new();
        let root = world.spawn(Name::new("Lantern")).id();
        let child = world.spawn((Name::new("Lantern"), ChildOf(root))).id();
        let _ = child;

        let result = resolve_entity(&mut world, &EntityRef::Name("Lantern".into()));
        assert_eq!(result.unwrap(), root);
    }

    #[test]
    fn resolve_entity_falls_back_to_child() {
        let mut world = World::new();
        let parent = world.spawn(Name::new("Parent")).id();
        let child = world.spawn((Name::new("ChildOnly"), ChildOf(parent))).id();

        let result = resolve_entity(&mut world, &EntityRef::Name("ChildOnly".into()));
        assert_eq!(result.unwrap(), child);
    }

    #[test]
    fn resolve_entity_root_preferred_regardless_of_spawn_order() {
        let mut world = World::new();
        let parent = world.spawn(Name::new("Parent")).id();
        let child = world.spawn((Name::new("Lamp"), ChildOf(parent))).id();
        let _ = child;
        // Root entity spawned after child - should still be preferred
        let root = world.spawn(Name::new("Lamp")).id();

        let result = resolve_entity(&mut world, &EntityRef::Name("Lamp".into()));
        assert_eq!(result.unwrap(), root);
    }

    #[test]
    fn resolve_entity_by_name_found() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Hero")).id();
        let result = resolve_entity(&mut world, &EntityRef::Name("Hero".into())).unwrap();
        assert_eq!(result, entity);
    }

    #[test]
    fn resolve_entity_by_name_not_found() {
        let mut world = World::new();
        let result = resolve_entity(&mut world, &EntityRef::Name("Ghost".into()));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_entity_by_id_found() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let result = resolve_entity(&mut world, &EntityRef::Id(entity.to_bits())).unwrap();
        assert_eq!(result, entity);
    }

    #[test]
    fn resolve_entity_by_id_not_found() {
        let mut world = World::new();
        let error = resolve_entity(&mut world, &EntityRef::Id(999999)).unwrap_err();
        assert_eq!(error.code, ErrorCode::NotFound);
    }

    #[test]
    fn resolve_entity_by_id_with_zero_index_returns_not_found_without_panicking() {
        // A caller-supplied id whose low 32 bits are zero is an invalid EntityIndex;
        // `Entity::from_bits` would panic and unwind through the exclusive
        // control-poll system. The handler must return NotFound instead of crashing.
        let mut world = World::new();
        for bits in [0u64, 1u64 << 32, 0xFFFF_FFFF_0000_0000] {
            let err = resolve_entity(&mut world, &EntityRef::Id(bits)).unwrap_err();
            assert!(matches!(err.code, ErrorCode::NotFound), "id {bits:#x}");
        }
    }

    #[test]
    fn despawn_entity_by_id_success() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let result = despawn_entity(&mut world, EntityRef::Id(entity.to_bits())).unwrap();
        assert_eq!(result["despawned"], true);
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn despawn_entity_by_name_success() {
        let mut world = World::new();
        world.spawn(Name::new("Target"));
        let result = despawn_entity(&mut world, EntityRef::Name("Target".into())).unwrap();
        assert_eq!(result["despawned"], true);
    }

    #[test]
    fn despawn_rejects_resource_entity() {
        let mut world = World::new();
        world.init_resource::<ClearColor>();
        let entity = world
            .query_filtered::<Entity, With<IsResource>>()
            .iter(&world)
            .next()
            .expect("init_resource should create a resource entity");

        let error = despawn_entity(&mut world, EntityRef::Id(entity.to_bits()))
            .expect_err("resource entity despawn must be rejected");

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(world.get_entity(entity).is_ok());
        assert!(world.get_resource::<ClearColor>().is_some());
    }

    #[test]
    fn despawn_rejects_resource_entity_in_subtree() {
        let mut world = World::new();
        world.init_resource::<ClearColor>();
        let resource_entity = world
            .query_filtered::<Entity, With<IsResource>>()
            .iter(&world)
            .next()
            .expect("init_resource should create a resource entity");
        let parent = world.spawn(Name::new("Parent")).id();
        world.entity_mut(parent).add_child(resource_entity);

        let error = despawn_entity(&mut world, EntityRef::Id(parent.to_bits()))
            .expect_err("cascade into a resource entity must be rejected");

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(world.get_resource::<ClearColor>().is_some());
    }

    #[test]
    fn despawn_entity_not_found() {
        let mut world = World::new();
        let result = despawn_entity(&mut world, EntityRef::Id(999999));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn despawn_entity_recursive_removes_children() {
        let mut world = World::new();
        let parent = world.spawn(Name::new("Parent")).id();
        let child1 = world.spawn((Name::new("Child1"), ChildOf(parent))).id();
        let child2 = world.spawn((Name::new("Child2"), ChildOf(parent))).id();
        let grandchild = world.spawn((Name::new("Grandchild"), ChildOf(child1))).id();

        let result = despawn_entity(&mut world, EntityRef::Id(parent.to_bits())).unwrap();
        assert_eq!(result["despawned"], true);

        // Parent and all descendants gone; no orphans with stale ChildOf.
        assert!(world.get_entity(parent).is_err());
        assert!(world.get_entity(child1).is_err());
        assert!(world.get_entity(child2).is_err());
        assert!(world.get_entity(grandchild).is_err());
    }

    #[test]
    fn despawn_entity_by_name_not_found() {
        let mut world = World::new();
        let result = despawn_entity(&mut world, EntityRef::Name("NonExistent".into()));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn has_embedded_errors_detects_errors() {
        let val = serde_json::json!({
            "entity_id": 42,
            "errors": ["field not found"],
        });
        assert!(has_embedded_errors(&val));
    }

    #[test]
    fn has_embedded_errors_empty_errors_is_false() {
        let val = serde_json::json!({
            "entity_id": 42,
            "errors": [],
        });
        assert!(!has_embedded_errors(&val));
    }

    #[test]
    fn has_embedded_errors_no_errors_key_is_false() {
        let val = serde_json::json!({
            "entity_id": 42,
            "components_added": ["Transform"],
        });
        assert!(!has_embedded_errors(&val));
    }

    #[test]
    fn world_despawn_vs_entity_mut_despawn_baseline() {
        // Pin the Bevy contract our despawn_entity handler relies on:
        // both `World::despawn` AND `EntityWorldMut::despawn` walk the
        // `Children` relationship recursively. If a future Bevy release
        // changes World::despawn back to non-recursive, despawn_entity
        // would need to switch to entity_mut(e).despawn() to preserve the
        // user-visible recursive behavior.
        let mut world = World::new();
        let parent_a = world.spawn(Name::new("ParentA")).id();
        let child_a = world.spawn((Name::new("ChildA"), ChildOf(parent_a))).id();

        let _ = world.despawn(parent_a);

        assert!(world.get_entity(parent_a).is_err());
        assert!(
            world.get_entity(child_a).is_err(),
            "World::despawn(parent) is recursive in Bevy 0.18: child should also be despawned"
        );

        let parent_b = world.spawn(Name::new("ParentB")).id();
        let child_b = world.spawn((Name::new("ChildB"), ChildOf(parent_b))).id();
        assert!(world.get::<Children>(parent_b).is_some());

        world.entity_mut(parent_b).despawn();

        assert!(world.get_entity(parent_b).is_err());
        assert!(world.get_entity(child_b).is_err());
    }
}
