use std::{any::TypeId, collections::HashSet};

use bevy::{
    ecs::{entity::Entity, resource::IsResource, world::World},
    log::info as log_info,
    prelude::{Resource, With, Without},
    time::{Time, Virtual},
    window::{Monitor, Window},
};
#[cfg(test)]
use pybevy_core::PluginIdentity;

use crate::{BaseEntitySet, Retained, runtime::ReloadRuntime};

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
/// including Bevy side-effect entities (e.g., `PointerId`).
pub fn clear_world_state<R: ReloadRuntime>(world: &mut World, runtime: &mut R, verbose: bool) {
    // Capture user-set Time knobs before any clear step can reset them.
    let virtual_time_state: Option<(bool, f32)> = world
        .get_resource::<Time<Virtual>>()
        .map(|t| (t.is_paused(), t.relative_speed()));

    // Despawn all entities not in the base set and not explicitly retained.
    let base = world
        .get_resource::<BaseEntitySet>()
        .map(|b| b.entities.clone())
        .unwrap_or_default();

    // Collect retained entities (editor camera, debug overlays, etc.)
    let retained: HashSet<Entity> = world
        .query_filtered::<Entity, With<Retained>>()
        .iter(world)
        .collect();

    // Resources are stored as entities; exclude them via the
    // `IsResource` marker so the cleanup never despawns resource-entities
    // (doing so triggers a panic when later commands touch them).
    // `Window`/`Monitor` entities are also off-limits: winit creates them
    // when the event loop starts (after the baseline snapshot), and
    // despawning the primary window exits the app via `exit_on_all_closed`.
    let to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, (Without<IsResource>, Without<Window>, Without<Monitor>)>()
        .iter(world)
        .filter(|e| !base.contains(e) && !retained.contains(e))
        .collect();

    // Count scene and resource entities with the same filter used for the
    // "remaining" log below, so the before/after numbers are comparable.
    let live_scene = world
        .query_filtered::<Entity, Without<IsResource>>()
        .iter(world)
        .count();
    let resource_entities = world
        .query_filtered::<Entity, With<IsResource>>()
        .iter(world)
        .count();
    log_info!(
        "[hot-reload] clear_world_state: despawning {} of {} scene entities ({} retained, {} resource-entities untouched)...",
        to_despawn.len(),
        live_scene,
        retained.len(),
        resource_entities
    );

    if verbose {
        eprintln!(
            "   → Despawning {} of {} scene entities",
            to_despawn.len(),
            live_scene
        );
    }

    for entity in to_despawn {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    let live_after = world
        .query_filtered::<Entity, Without<IsResource>>()
        .iter(world)
        .count();
    log_info!(
        "[hot-reload] clear_world_state: {} scene entities remaining",
        live_after
    );

    // Don't explicitly clear programmatic assets. Entity despawn already drops
    // all Handle<T> components, and Bevy's `track_assets` system GC's orphaned
    // assets on the next frame. Explicitly removing assets here would destroy
    // Bevy-internal programmatic assets (e.g. TextureAtlasLayout) whose handles
    // are still held by preserved base entities, causing dangling-handle panics
    // in render systems like bevy_ui_render.

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

    // Restore captured Time knobs onto a fresh Time (elapsed=0 by construction).
    if let Some((was_paused, speed)) = virtual_time_state {
        let mut fresh = Time::<Virtual>::default();
        fresh.set_relative_speed(speed);
        if was_paused {
            fresh.pause();
        }
        world.insert_resource(fresh);

        if verbose {
            eprintln!(
                "   → Reset virtual time to zero (preserved speed={} paused={})",
                speed, was_paused
            );
        }
    }
    // Default Time wraps the real wall clock; preserving it isn't meaningful
    // but rebuilding it throws off accumulated render frame deltas. Leave it.
}

/// Fold every currently-live entity into the `BaseEntitySet` baseline.
///
/// The initial baseline is captured before `app.run()`, but winit creates
/// more engine entities once the event loop starts (`Monitor`s, a11y).
/// Running this right before the first user Startup system folds those into
/// the baseline so Full reloads never despawn them.
pub fn extend_base_entity_set(world: &mut World) {
    let entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();
    let Some(mut base) = world.get_resource_mut::<BaseEntitySet>() else {
        return;
    };
    base.entities.extend(entities);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::{
        asset::{Asset, AssetServer, Assets, RenderAssetUsages},
        mesh::{Mesh, Mesh3d, PrimitiveTopology},
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
        fn defs_fingerprint(&self, _defs: &()) -> crate::runtime::DefsFingerprint {
            crate::runtime::DefsFingerprint::default()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
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
    struct Marker;

    fn live_entity_count(world: &mut World) -> usize {
        // Resources are stored as entities; exclude them via the
        // `IsResource` marker so counts reflect only scene entities, matching
        // what the production `clear_world_state` cleanup operates on.
        world
            .query_filtered::<Entity, Without<IsResource>>()
            .iter(world)
            .count()
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

        world.spawn(Mesh3d(handle));

        assert_eq!(live_count::<Mesh>(&world), 1);

        // Despawn entities. Their Handle<Mesh> components are dropped.
        // The asset still exists in Assets<Mesh> (Bevy's track_assets GC
        // processes orphaned assets on the next frame, not synchronously),
        // but the entity no longer holds a reference to it.
        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert_eq!(
            live_count::<Mesh>(&world),
            1,
            "orphaned asset remains until Bevy's track_assets GC runs"
        );
    }

    #[test]
    fn despawns_entities_not_in_base_set() {
        let mut world = World::new();
        // Bevy internal entity (in base set) - should survive
        let internal = world.spawn(Marker).id();
        insert_base_set(&mut world, vec![internal]);

        // User entities (not in base set) - should be despawned
        let user1 = world.spawn(Marker).id();
        let user2 = world.spawn(Marker).id();

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
    fn retained_entities_survive_reload() {
        let mut world = World::new();
        let internal = world.spawn(Marker).id();
        insert_base_set(&mut world, vec![internal]);

        // User entity (should be despawned)
        let user = world.spawn(Marker).id();
        // Retained entity (should survive even though not in base set)
        let editor_cam = world.spawn((crate::Retained, Marker)).id();

        assert_eq!(live_entity_count(&mut world), 3);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert_eq!(live_entity_count(&mut world), 2);
        assert!(
            world.get_entity(internal).is_ok(),
            "base entity should survive"
        );
        assert!(
            world.get_entity(user).is_err(),
            "user entity should be despawned"
        );
        assert!(
            world.get_entity(editor_cam).is_ok(),
            "retained entity should survive reload"
        );
    }

    #[test]
    fn window_and_monitor_entities_survive_cleanup() {
        use bevy::{math::IVec2, window::OnMonitor};

        let mut world = World::new();
        insert_base_set(&mut world, vec![]);

        // Simulates winit state after event-loop start: Monitor spawned
        // post-baseline, Window related to it. Monitor's `HasWindows` is a
        // `linked_spawn` relationship, so despawning the monitor would
        // cascade-despawn the window and exit the app.
        let monitor = world
            .spawn(Monitor {
                name: None,
                physical_height: 1080,
                physical_width: 1920,
                physical_position: IVec2::ZERO,
                refresh_rate_millihertz: None,
                scale_factor: 1.0,
                video_modes: vec![],
            })
            .id();
        let window = world.spawn((Window::default(), OnMonitor(monitor))).id();
        let user = world.spawn(Marker).id();

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert!(
            world.get_entity(monitor).is_ok(),
            "monitor must survive cleanup; winit never respawns it"
        );
        assert!(
            world.get_entity(window).is_ok(),
            "window must survive cleanup; despawning it exits the app"
        );
        assert!(world.get_entity(user).is_err(), "user entity despawned");
    }

    #[test]
    fn extend_base_entity_set_folds_live_entities() {
        let mut world = World::new();
        insert_base_set(&mut world, vec![]);

        // Engine entity created after the initial snapshot (e.g., at
        // event-loop start) but before user Startup.
        let engine = world.spawn(Marker).id();
        extend_base_entity_set(&mut world);
        let user = world.spawn(Marker).id();

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert!(
            world.get_entity(engine).is_ok(),
            "entity alive at extension time joins the baseline"
        );
        assert!(
            world.get_entity(user).is_err(),
            "entity spawned after extension is still despawned"
        );
    }

    #[test]
    fn despawns_all_non_base_entities() {
        let mut world = World::new();
        // Base entity (plugin-init)
        let internal = world.spawn(Marker).id();
        insert_base_set(&mut world, vec![internal]);

        let user = world.spawn(Marker).id();
        // Bevy side-effect entity (e.g., PointerId)
        let side_effect = world.spawn(Marker).id();

        assert_eq!(live_entity_count(&mut world), 3);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert_eq!(live_entity_count(&mut world), 1);
        assert!(world.get_entity(internal).is_ok(), "base entity survives");
        assert!(world.get_entity(user).is_err(), "user entity despawned");
        assert!(
            world.get_entity(side_effect).is_err(),
            "side-effect entity must also be despawned"
        );
    }

    #[test]
    fn recursive_despawn_removes_children() {
        let mut world = World::new();
        // Internal entity - should survive
        let internal = world.spawn(Marker).id();
        insert_base_set(&mut world, vec![internal]);

        let parent = world
            .spawn(Marker)
            .with_children(|cb| {
                cb.spawn(Marker);
                cb.spawn(Marker).with_children(|cb2| {
                    cb2.spawn(Marker);
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
        let internal = world.spawn(Marker).id();
        insert_base_set(&mut world, vec![internal]);

        // Simulate a scene like chair-race: multiple parents each with children
        for _i in 0..4 {
            world.spawn(Marker).with_children(|cb| {
                for _ in 0..10 {
                    cb.spawn(Marker);
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
        fn defs_fingerprint(&self, _defs: &()) -> crate::runtime::DefsFingerprint {
            crate::runtime::DefsFingerprint::default()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
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

    #[test]
    fn clear_world_state_resets_virtual_time_elapsed() {
        let mut world = World::new();

        world.insert_resource(Time::<Virtual>::default());
        let mut time_virt = world.resource_mut::<Time<Virtual>>();
        time_virt.advance_by(Duration::from_secs(5));
        drop(time_virt);

        let elapsed_before = world.resource::<Time<Virtual>>().elapsed_secs();
        assert!(elapsed_before >= 5.0, "time should have advanced");

        clear_world_state(&mut world, &mut NoopRuntime, false);

        let elapsed_after = world.resource::<Time<Virtual>>().elapsed_secs();
        assert_eq!(
            elapsed_after, 0.0,
            "virtual elapsed time should reset to zero on full reload (matches \"fresh play\" mental model)"
        );
    }

    #[test]
    fn clear_world_state_preserves_virtual_time_pause() {
        let mut world = World::new();

        world.insert_resource(Time::<Virtual>::default());
        let mut time_virt = world.resource_mut::<Time<Virtual>>();
        time_virt.pause();
        drop(time_virt);
        assert!(world.resource::<Time<Virtual>>().is_paused());

        clear_world_state(&mut world, &mut NoopRuntime, false);

        assert!(
            world.resource::<Time<Virtual>>().is_paused(),
            "pause state should survive clear_world_state"
        );
    }

    #[test]
    fn clear_world_state_preserves_virtual_time_relative_speed() {
        let mut world = World::new();

        world.insert_resource(Time::<Virtual>::default());
        let mut time_virt = world.resource_mut::<Time<Virtual>>();
        time_virt.set_relative_speed(0.25);
        drop(time_virt);

        clear_world_state(&mut world, &mut NoopRuntime, false);

        let speed = world.resource::<Time<Virtual>>().relative_speed();
        assert!(
            (speed - 0.25).abs() < 1e-6,
            "relative_speed should survive clear_world_state (got {})",
            speed
        );
    }

    /// Runtime that mirrors real Pyo3ReloadRuntime: clear_native_resources
    /// resets bridged Bevy-plugin resources (incl. Time<Virtual>) to default.
    struct TimeResetRuntime;

    impl ReloadRuntime for TimeResetRuntime {
        type Defs = ();
        type SystemHandle = ();

        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn defs_fingerprint(&self, _defs: &()) -> crate::runtime::DefsFingerprint {
            crate::runtime::DefsFingerprint::default()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
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
            if world.contains_resource::<Time<Virtual>>() {
                initial.insert(TypeId::of::<Time<Virtual>>());
            }
            initial
        }
        fn clear_native_resources(
            &self,
            world: &mut World,
            initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
            if initial.contains(&TypeId::of::<Time<Virtual>>()) {
                world.insert_resource(Time::<Virtual>::default());
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

    /// clear_native_resources resets Time<Virtual> to default for any bridged
    /// Bevy-plugin resource; the capture-and-restore must run around that step,
    /// not after, or pause+speed flip back to defaults.
    #[test]
    fn clear_world_state_preserves_time_across_native_reset() {
        let mut world = World::new();

        // Time<Virtual> exists at snapshot time (mirrors TimePlugin in real app).
        world.insert_resource(Time::<Virtual>::default());

        let runtime = TimeResetRuntime;
        let initial = runtime.snapshot_native_resources(&world);
        world.insert_resource(NativeResourceSnapshot { initial });

        // User code sets pause and speed.
        let mut time_virt = world.resource_mut::<Time<Virtual>>();
        time_virt.pause();
        time_virt.set_relative_speed(0.25);
        time_virt.advance_by(Duration::from_secs(7));
        drop(time_virt);

        let mut runtime = TimeResetRuntime;
        clear_world_state(&mut world, &mut runtime, false);

        let after = world.resource::<Time<Virtual>>();
        assert!(
            after.is_paused(),
            "pause must survive clear_native_resources reset"
        );
        assert!(
            (after.relative_speed() - 0.25).abs() < 1e-6,
            "relative_speed must survive clear_native_resources reset (got {})",
            after.relative_speed()
        );
        assert_eq!(
            after.elapsed_secs(),
            0.0,
            "elapsed should reset to zero on full reload"
        );
    }

    /// Regression test for Camera2d hot-reload crash.
    ///
    /// When a Bevy-internal (base) entity holds a Handle<T> to a programmatic
    /// asset, reload must not remove that asset. Otherwise the base entity has
    /// a dangling handle and render systems panic:
    ///
    ///   bevy_ui_render/src/lib.rs:976: called `Option::unwrap()` on a `None` value
    ///
    /// The crash pattern: bevy_ui_render holds a Handle<TextureAtlasLayout> on a
    /// preserved entity; the old cleanup code deleted the TextureAtlasLayout
    /// because it had no file path (it's `not_loadable`). Uses Mesh as a
    /// stand-in since the cleanup logic is identical for all asset types.
    #[test]
    fn base_entity_asset_handle_valid_after_reload() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();

        // Simulate Bevy plugin init: create a programmatic asset and a base entity
        // that holds a handle to it (like bevy_ui_render's internal TextureAtlasLayout).
        let handle = world.resource_mut::<Assets<Mesh>>().add(test_mesh());
        let base_entity = world.spawn((Marker, Mesh3d(handle.clone()))).id();
        insert_base_set(&mut world, vec![base_entity]);

        // Simulate user code: create a user asset + entity (not in base set)
        let user_handle = world.resource_mut::<Assets<Mesh>>().add(test_mesh());
        world.spawn((Marker, Mesh3d(user_handle)));

        assert_eq!(live_count::<Mesh>(&world), 2);

        // Full reload: despawn user entities (drops their asset handles).
        // clear_world_state no longer explicitly removes assets. Bevy's
        // track_assets GC will clean up orphaned assets on the next frame.
        clear_world_state(&mut world, &mut NoopRuntime, false);

        // Base entity must survive (it's in the BaseEntitySet)
        assert!(
            world.get_entity(base_entity).is_ok(),
            "base entity must survive reload"
        );

        // Both assets still exist in Assets<Mesh>: the base entity's asset has
        // a live handle, and the user's orphaned asset hasn't been GC'd yet
        // (Bevy's track_assets runs on the next frame, not synchronously).
        // The important thing is the base entity's handle is NOT dangling.
        let mesh3d = world.get::<Mesh3d>(base_entity).unwrap();
        let assets = world.resource::<Assets<Mesh>>();
        assert!(
            assets.get(&mesh3d.0).is_some(),
            "base entity's asset handle must remain valid after reload"
        );
    }
}
