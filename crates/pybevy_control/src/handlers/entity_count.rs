//! Canonical scene-entity enumeration.
//!
//! Resources are stored as entities in Bevy, so every MCP endpoint that reports
//! scene entities MUST exclude them (`Without<IsResource>`). Centralizing that
//! filter here keeps `get_performance.entity_count`, `get_scene_summary`,
//! `scene://entities`, `query_entities` and `reload_and_capture` in agreement on
//! a single canonical value, and stops a new endpoint from re-deriving the query
//! and drifting — as `reload_and_capture` once did by reporting the raw
//! `world.entities().len()` (which over-counts by the resource-backing entities).

use bevy::ecs::{entity::Entity, prelude::Without, resource::IsResource, world::World};

/// All real scene entities, with resource-backing entities excluded.
///
/// Use this when you need the entity list itself (to inspect components, group,
/// etc.). If you only need the number, prefer [`scene_entity_count`], which
/// avoids allocating the `Vec`.
pub(crate) fn scene_entities(world: &mut World) -> Vec<Entity> {
    world
        .query_filtered::<Entity, Without<IsResource>>()
        .iter(world)
        .collect()
}

/// The canonical scene entity count — resource-backing entities excluded.
///
/// This is the single value that all `entity_count` / `total_entities` fields
/// across the MCP surface must report.
pub(crate) fn scene_entity_count(world: &mut World) -> u64 {
    world
        .query_filtered::<Entity, Without<IsResource>>()
        .iter(world)
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_spawned_entities() {
        let mut world = World::new();
        world.spawn_empty();
        world.spawn_empty();
        world.spawn_empty();
        assert_eq!(scene_entity_count(&mut world), 3);
        assert_eq!(scene_entities(&mut world).len(), 3);
    }

    #[test]
    fn excludes_resource_backing_entities() {
        // Resources are entity-backed in Bevy; inserting one must not inflate the
        // scene entity count. `world.entities().len()` (the raw total) would grow
        // here — the canonical count must not.
        let mut world = World::new();
        world.spawn_empty();
        world.spawn_empty();

        let before = scene_entity_count(&mut world);
        world.insert_resource(Marker(7));
        let raw_total = world.entities().len() as u64;

        assert_eq!(
            scene_entity_count(&mut world),
            before,
            "inserting a resource must not change the canonical scene entity count",
        );
        assert!(
            raw_total > before,
            "sanity: the raw entity table should grow when a resource is inserted \
             (otherwise this test can't distinguish the filter)",
        );
        assert_eq!(scene_entities(&mut world).len() as u64, before);
    }

    #[derive(bevy::prelude::Resource)]
    struct Marker(#[allow(dead_code)] u32);
}
