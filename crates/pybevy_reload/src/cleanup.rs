use std::{any::TypeId, collections::HashSet};

use bevy::{
    ecs::{entity::Entity, world::World},
    log::info as log_info,
    prelude::Resource,
    time::{Time, Virtual},
};
use pybevy_core::asset_cleanup::clear_all_programmatic_assets;

use crate::{BaseEntitySet, runtime::ReloadRuntime};

/// Snapshot of which bridged native resources existed before any user code ran.
///
/// Captured once (before the first reload) and persists across reloads.
/// Used to distinguish Bevy-plugin resources (reset to default) from
/// user-inserted resources (remove entirely) during Full reload.
#[derive(Resource, Default)]
pub struct NativeResourceSnapshot {
    pub initial: HashSet<TypeId>,
}

/// Clear all user-spawned entities, programmatic assets, and custom resources.
/// Used by both the Full reload path and escalation from Partial to Full.
///
/// Uses the `BaseEntitySet` to determine which entities are Bevy-internal
/// (plugin-init) and should be preserved. Everything else is despawned,
/// including Bevy side-effect entities (e.g., `PointerId`) that don't carry
/// the `HotReloadable` marker.
pub fn clear_world_state<R: ReloadRuntime>(world: &mut World, runtime: &mut R, verbose: bool) {
    // Despawn all entities not in the base set (plugin-init entities).
    let base = world
        .get_resource::<BaseEntitySet>()
        .map(|b| b.entities.clone())
        .unwrap_or_default();
    let to_despawn: Vec<Entity> = world
        .query::<Entity>()
        .iter(world)
        .filter(|e| !base.contains(e))
        .collect();

    let live_before = base.len() + to_despawn.len();
    log_info!(
        "[hot-reload] clear_world_state: {} base / {} live entities, despawning {}...",
        base.len(),
        live_before,
        to_despawn.len()
    );

    if verbose {
        eprintln!(
            "   → Despawning {} entities ({} base preserved)",
            to_despawn.len(),
            base.len()
        );
    }

    for entity in to_despawn {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    let live_after = world.query::<Entity>().iter(world).count();
    log_info!(
        "[hot-reload] clear_world_state: {} live entities remaining",
        live_after
    );

    // Clear programmatic assets (preserve file-loaded)
    if verbose {
        eprintln!("   → Clearing programmatic assets (preserving file-loaded)");
    }

    clear_all_programmatic_assets(world, verbose);

    // Clear custom runtime resources (preserves built-in and HotReloadControl)
    if verbose {
        eprintln!("   → Clearing custom resources");
    }
    runtime.clear_custom_resources(world, verbose);

    // Reset/remove native bridged resources based on initial snapshot
    if let Some(snapshot) = world.get_resource::<NativeResourceSnapshot>() {
        let initial = snapshot.initial.clone();
        runtime.clear_native_resources(world, &initial, verbose);
    }

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
    use crate::HotReloadable;

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
        fn snapshot_native_resources(&self, _world: &World) -> HashSet<TypeId> {
            HashSet::new()
        }
        fn clear_native_resources(
            &self,
            _world: &mut World,
            _initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
        }
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

    /// Helper: insert a BaseEntitySet containing the given entities.
    fn insert_base_set(world: &mut World, entities: Vec<Entity>) {
        world.insert_resource(crate::BaseEntitySet {
            entities: entities.into_iter().collect(),
        });
    }

    #[test]
    fn entity_despawn_drops_asset_handles() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        insert_base_set(&mut world, vec![]); // no base entities

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
    fn despawns_entities_not_in_base_set() {
        let mut world = World::new();
        // Bevy internal entity (in base set) - should survive
        let internal = world.spawn(Marker("internal")).id();
        insert_base_set(&mut world, vec![internal]);

        // User entities (not in base set) - should be despawned
        let user1 = world.spawn((HotReloadable, Marker("user1"))).id();
        let user2 = world.spawn((HotReloadable, Marker("user2"))).id();

        assert_eq!(live_entity_count(&mut world), 3);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert_eq!(live_entity_count(&mut world), 1);
        assert!(
            world.get_entity(internal).is_ok(),
            "base entity should survive"
        );
        assert!(
            world.get_entity(user1).is_err(),
            "non-base entity should be despawned"
        );
        assert!(
            world.get_entity(user2).is_err(),
            "non-base entity should be despawned"
        );
    }

    #[test]
    fn despawns_side_effect_entities_without_hotreloadable() {
        let mut world = World::new();
        // Base entity (plugin-init)
        let internal = world.spawn(Marker("internal")).id();
        insert_base_set(&mut world, vec![internal]);

        // User entity with HotReloadable
        let user = world.spawn((HotReloadable, Marker("user_camera"))).id();
        // Bevy side-effect entity WITHOUT HotReloadable (e.g., PointerId)
        let side_effect = world.spawn(Marker("pointer_id")).id();

        assert_eq!(live_entity_count(&mut world), 3);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert_eq!(live_entity_count(&mut world), 1);
        assert!(world.get_entity(internal).is_ok(), "base entity survives");
        assert!(world.get_entity(user).is_err(), "user entity despawned");
        assert!(
            world.get_entity(side_effect).is_err(),
            "side-effect entity without HotReloadable must also be despawned"
        );
    }

    #[test]
    fn recursive_despawn_removes_children() {
        let mut world = World::new();
        // Internal entity - should survive
        let internal = world.spawn(Marker("internal")).id();
        insert_base_set(&mut world, vec![internal]);

        // Parent (not in base set), children without HotReloadable
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
            "only base entity should survive - parent + all descendants must be recursively despawned"
        );
        assert!(world.get_entity(parent).is_err());
    }

    #[test]
    fn multiple_parents_with_children() {
        let mut world = World::new();
        let internal = world.spawn(Marker("internal")).id();
        insert_base_set(&mut world, vec![internal]);

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
            "only base entity should survive"
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

    /// Resource that simulates a Bevy-plugin default (present at snapshot time).
    #[derive(Resource, Default, Debug, PartialEq)]
    struct PluginRes(u32);

    /// Resource that simulates a user-only insertion (not present at snapshot time).
    #[derive(Resource, Default, Debug, PartialEq)]
    struct UserOnlyRes(u32);

    /// Runtime that implements native resource reset for test resources.
    struct NativeResetRuntime;

    impl ReloadRuntime for NativeResetRuntime {
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
        fn snapshot_native_resources(&self, world: &World) -> HashSet<TypeId> {
            let mut initial = HashSet::new();
            if world.contains_resource::<PluginRes>() {
                initial.insert(TypeId::of::<PluginRes>());
            }
            if world.contains_resource::<UserOnlyRes>() {
                initial.insert(TypeId::of::<UserOnlyRes>());
            }
            initial
        }
        fn clear_native_resources(
            &self,
            world: &mut World,
            initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
            // PluginRes
            if initial.contains(&TypeId::of::<PluginRes>()) {
                world.insert_resource(PluginRes::default());
            } else if world.contains_resource::<PluginRes>() {
                world.remove_resource::<PluginRes>();
            }
            // UserOnlyRes
            if initial.contains(&TypeId::of::<UserOnlyRes>()) {
                world.insert_resource(UserOnlyRes::default());
            } else if world.contains_resource::<UserOnlyRes>() {
                world.remove_resource::<UserOnlyRes>();
            }
        }
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

    #[test]
    fn native_resource_reset_to_default_on_full_reload() {
        let mut world = World::new();

        // PluginRes is present at snapshot time (simulates Bevy-plugin default)
        world.insert_resource(PluginRes(42));

        // Take snapshot
        let runtime = NativeResetRuntime;
        let initial = runtime.snapshot_native_resources(&world);
        world.insert_resource(NativeResourceSnapshot { initial });

        // User code modifies PluginRes and adds UserOnlyRes
        world.insert_resource(PluginRes(999));
        world.insert_resource(UserOnlyRes(123));

        // Full reload cleanup
        let mut runtime = NativeResetRuntime;
        clear_world_state(&mut world, &mut runtime, false);

        // PluginRes should be reset to default (0), not 999
        assert_eq!(
            world.resource::<PluginRes>().0,
            0,
            "Initial resource should be reset to T::default()"
        );

        // UserOnlyRes should be removed entirely
        assert!(
            !world.contains_resource::<UserOnlyRes>(),
            "User-only resource should be removed on reload"
        );
    }

    #[test]
    fn native_resource_removed_by_user_is_reinserted_on_reload() {
        let mut world = World::new();

        // PluginRes present at snapshot time
        world.insert_resource(PluginRes(42));

        let runtime = NativeResetRuntime;
        let initial = runtime.snapshot_native_resources(&world);
        world.insert_resource(NativeResourceSnapshot { initial });

        // User code removes the Bevy-plugin resource
        world.remove_resource::<PluginRes>();
        assert!(!world.contains_resource::<PluginRes>());

        // Full reload cleanup should re-insert the default
        let mut runtime = NativeResetRuntime;
        clear_world_state(&mut world, &mut runtime, false);

        assert!(
            world.contains_resource::<PluginRes>(),
            "Removed initial resource should be re-inserted with default"
        );
        assert_eq!(world.resource::<PluginRes>().0, 0);
    }

    #[test]
    fn snapshot_only_contains_resources_present_at_capture_time() {
        let mut world = World::new();

        // Only PluginRes exists at snapshot time
        world.insert_resource(PluginRes(1));

        let runtime = NativeResetRuntime;
        let initial = runtime.snapshot_native_resources(&world);

        assert!(initial.contains(&TypeId::of::<PluginRes>()));
        assert!(
            !initial.contains(&TypeId::of::<UserOnlyRes>()),
            "Resources not present at snapshot time should not be in initial set"
        );
    }

    #[test]
    fn no_native_reset_without_snapshot() {
        let mut world = World::new();

        // Insert resources but don't create a snapshot
        world.insert_resource(PluginRes(42));
        world.insert_resource(UserOnlyRes(99));

        // clear_world_state without snapshot should not touch native resources
        let mut runtime = NativeResetRuntime;
        clear_world_state(&mut world, &mut runtime, false);

        // Both resources should be untouched (no snapshot = empty initial set)
        assert_eq!(world.resource::<PluginRes>().0, 42);
        assert_eq!(world.resource::<UserOnlyRes>().0, 99);
    }
}
