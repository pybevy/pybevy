use bevy::{
    ecs::{entity::Entity, query::With, world::World},
    time::{Time, Virtual},
};

use crate::{HotReloadable, runtime::ReloadRuntime};

/// Clear all user-spawned entities, programmatic assets, and custom resources.
/// Used by both the Full reload path and escalation from Partial to Full.
pub fn clear_world_state<R: ReloadRuntime>(world: &mut World, runtime: &mut R, verbose: bool) {
    // Despawn all entities marked with HotReloadable
    let mut query = world.query_filtered::<Entity, With<HotReloadable>>();
    let all_hotreloadable: Vec<Entity> = query.iter(world).collect();

    if verbose {
        eprintln!("   → Despawning {} user entities", all_hotreloadable.len());
    }

    for entity in all_hotreloadable {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    // Clear programmatic assets (preserve file-loaded)
    if verbose {
        eprintln!("   → Clearing programmatic assets (preserving file-loaded)");
    }

    pybevy_core::asset_cleanup::clear_all_programmatic_assets(world, verbose);

    // Clear custom runtime resources (preserves built-in and HotReloadControl)
    if verbose {
        eprintln!("   → Clearing custom resources");
    }
    runtime.clear_custom_resources(world, verbose);

    // Reset game time so elapsed_secs() starts from zero after full reload.
    if world.get_resource::<Time<Virtual>>().is_some() {
        world.insert_resource(Time::<Virtual>::default());
        if verbose {
            eprintln!("   → Reset virtual time to zero");
        }
    }
    if world.get_resource::<Time>().is_some() {
        world.insert_resource(Time::<()>::default());
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::{Asset, AssetServer, Assets, RenderAssetUsages},
        mesh::{Mesh, PrimitiveTopology},
        pbr::StandardMaterial,
        reflect::TypePath,
    };

    use super::*;

    /// Minimal ReloadRuntime for tests - all methods are no-ops.
    struct NoopRuntime;

    impl ReloadRuntime for NoopRuntime {
        type Defs = ();
        type SystemHandle = ();

        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn requires_escalation(&self, _defs: &()) -> Option<&'static str> {
            None
        }
        fn plugin_names(&self, _defs: &()) -> Vec<String> {
            vec![]
        }
        fn system_names(&self, _defs: &()) -> std::collections::HashSet<String> {
            std::collections::HashSet::new()
        }
        fn register_systems(
            &mut self,
            _world: &mut World,
            _defs: (),
            _gen: u32,
        ) -> Result<Vec<()>, ReloadError> {
            Ok(vec![])
        }
        fn register_resources(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_messages(
            &mut self,
            _world: &mut World,
            _defs: &(),
            _gen: u32,
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_observers(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_handles(&mut self, _world: &mut World, _gen: u32, _handles: Vec<()>) {}
        fn prune_messages(&mut self, _world: &mut World, _keep_after: u32) {}
        fn clear_custom_resources(&mut self, _world: &mut World, _verbose: bool) {}
        fn detect_system_delta(
            &mut self,
            _world: &mut World,
            _new: std::collections::HashSet<String>,
        ) -> Vec<String> {
            vec![]
        }
        fn clear_param_cache(&mut self) {}
        fn trigger_gc(&mut self) {}
        fn print_error(&self, _error: &ReloadError) {}
    }

    use bevy::ecs::component::Component;

    use crate::runtime::ReloadError;

    #[derive(Component)]
    struct Marker(&'static str);

    fn live_entity_count(world: &mut World) -> usize {
        world.query::<Entity>().iter(world).count()
    }

    /// Count live assets of a given type in the world.
    fn live_count<T: Asset>(world: &World) -> usize {
        world.get_resource::<Assets<T>>().map_or(0, |a| a.len())
    }

    /// Clear all assets of a given type from a world (test helper).
    /// Mirrors the logic of the macro-generated `clear_programmatic` method.
    fn clear_assets<T: Asset>(world: &mut World) {
        let ids_to_remove: Vec<_> = {
            let Some(asset_server) = world.get_resource::<AssetServer>() else {
                if let Some(mut assets) = world.get_resource_mut::<Assets<T>>() {
                    let handles: Vec<_> = assets.ids().map(|id| id.untyped()).collect();
                    for handle in handles {
                        assets.remove(handle.typed::<T>());
                    }
                }
                return;
            };
            let Some(assets) = world.get_resource::<Assets<T>>() else {
                return;
            };
            assets
                .ids()
                .filter(|id| asset_server.get_path(id.untyped()).is_none())
                .collect()
        };
        if let Some(mut assets) = world.get_resource_mut::<Assets<T>>() {
            for id in &ids_to_remove {
                assets.remove(*id);
            }
        }
    }

    fn test_mesh() -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
    }

    #[test]
    fn programmatic_mesh_assets_cleared() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();

        let mut assets = world.resource_mut::<Assets<Mesh>>();
        assets.add(test_mesh());
        assets.add(test_mesh());
        assets.add(test_mesh());
        drop(assets);

        assert_eq!(live_count::<Mesh>(&world), 3);

        clear_assets::<Mesh>(&mut world);

        assert_eq!(live_count::<Mesh>(&world), 0);
    }

    #[test]
    fn programmatic_material_assets_cleared() {
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();

        let mut assets = world.resource_mut::<Assets<StandardMaterial>>();
        assets.add(StandardMaterial::default());
        assets.add(StandardMaterial::default());
        drop(assets);

        assert_eq!(live_count::<StandardMaterial>(&world), 2);

        clear_assets::<StandardMaterial>(&mut world);

        assert_eq!(live_count::<StandardMaterial>(&world), 0);
    }

    #[test]
    fn entity_despawn_drops_asset_handles() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();

        let handle = world.resource_mut::<Assets<Mesh>>().add(test_mesh());

        // Spawn an entity holding HotReloadable, plus attach the handle as Mesh3d
        world.spawn((HotReloadable, bevy::mesh::Mesh3d(handle)));

        assert_eq!(live_count::<Mesh>(&world), 1);

        // Despawn entities, then clear assets
        clear_world_state(&mut world, &mut NoopRuntime, false);
        clear_assets::<Mesh>(&mut world);

        assert_eq!(live_count::<Mesh>(&world), 0);
    }

    #[test]
    fn despawns_hotreloadable_entities() {
        let mut world = World::new();
        // Bevy internal entity (no HotReloadable) - should survive
        let internal = world.spawn(Marker("internal")).id();
        // User entities with HotReloadable - should be despawned
        let user1 = world.spawn((HotReloadable, Marker("user1"))).id();
        let user2 = world.spawn((HotReloadable, Marker("user2"))).id();

        assert_eq!(live_entity_count(&mut world), 3);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert_eq!(live_entity_count(&mut world), 1);
        assert!(
            world.get_entity(internal).is_ok(),
            "internal entity should survive"
        );
        assert!(
            world.get_entity(user1).is_err(),
            "user entity should be despawned"
        );
        assert!(
            world.get_entity(user2).is_err(),
            "user entity should be despawned"
        );
    }

    #[test]
    fn recursive_despawn_removes_children() {
        let mut world = World::new();
        // Internal entity - should survive
        world.spawn(Marker("internal"));

        // Parent with HotReloadable, children without
        let parent = world
            .spawn((HotReloadable, Marker("parent")))
            .with_children(|cb| {
                cb.spawn(Marker("child1"));
                cb.spawn(Marker("child2")).with_children(|cb2| {
                    cb2.spawn(Marker("grandchild"));
                });
            })
            .id();

        // 1 internal + 1 parent + 2 children + 1 grandchild = 5
        assert_eq!(live_entity_count(&mut world), 5);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert_eq!(
            live_entity_count(&mut world),
            1,
            "only internal entity should survive - parent + all descendants must be recursively despawned"
        );
        assert!(world.get_entity(parent).is_err());
    }

    #[test]
    fn multiple_parents_with_children() {
        let mut world = World::new();
        let internal = world.spawn(Marker("internal")).id();

        // Simulate a scene like chair-race: multiple parents each with children
        for _i in 0..4 {
            world
                .spawn((HotReloadable, Marker("chair")))
                .with_children(|cb| {
                    for _ in 0..10 {
                        cb.spawn(Marker("part"));
                    }
                });
        }

        // 1 internal + 4 parents + 40 children = 45
        assert_eq!(live_entity_count(&mut world), 45);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert_eq!(
            live_entity_count(&mut world),
            1,
            "only internal entity should survive"
        );
        assert!(world.get_entity(internal).is_ok());
    }

    /// Asset type without a registered bridge is not cleaned up.
    /// Documents that only bridged types get automatic cleanup.
    #[derive(Asset, TypePath)]
    struct UnregisteredAsset;

    #[test]
    fn unbridged_asset_types_not_cleared() {
        let mut world = World::new();
        world.init_resource::<Assets<UnregisteredAsset>>();

        world
            .resource_mut::<Assets<UnregisteredAsset>>()
            .add(UnregisteredAsset);

        assert_eq!(live_count::<UnregisteredAsset>(&world), 1);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        // NOT cleaned up - no bridge registered for this type
        assert_eq!(live_count::<UnregisteredAsset>(&world), 1);
    }
}
