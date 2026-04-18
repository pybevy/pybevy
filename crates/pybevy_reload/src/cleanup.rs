use bevy::{
    animation::{AnimationClip, graph::AnimationGraph},
    asset::{Asset, AssetServer, Assets},
    ecs::{entity::Entity, query::With, world::World},
    image::Image,
    mesh::Mesh,
    pbr::StandardMaterial,
    scene::Scene,
    time::{Time, Virtual},
};

use crate::{HotReloadable, runtime::ReloadRuntime};

/// Clear only programmatically-created assets (those added via `assets.add()`).
///
/// File-loaded assets (loaded via `asset_server.load()`) are preserved so that
/// handles returned by the AssetServer remain valid after reload. Without this,
/// `asset_server.load("model.glb#Scene0")` returns a stale handle pointing to
/// cleared data because the AssetServer deduplicates by path.
fn clear_programmatic_assets<T: Asset>(world: &mut World, verbose: bool, name: &str) {
    // Collect IDs of programmatic assets (those without a path in AssetServer)
    let ids_to_remove: Vec<_> = {
        let Some(asset_server) = world.get_resource::<AssetServer>() else {
            // No AssetServer — clear all assets (no file-loaded assets to preserve)
            if let Some(mut assets) = world.get_resource_mut::<Assets<T>>() {
                let count = assets.len();
                if count > 0 {
                    let handles: Vec<_> = assets.ids().map(|id| id.untyped()).collect();
                    for handle in handles {
                        assets.remove(handle.typed::<T>());
                    }
                    if verbose {
                        eprintln!("      Cleared {} {} (no AssetServer)", count, name);
                    }
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

    if ids_to_remove.is_empty() {
        return;
    }

    if let Some(mut assets) = world.get_resource_mut::<Assets<T>>() {
        for id in &ids_to_remove {
            assets.remove(*id);
        }
    }

    if verbose {
        let preserved = world.get_resource::<Assets<T>>().map_or(0, |a| a.len());
        eprintln!(
            "      Cleared {} programmatic {} (preserved {} file-loaded)",
            ids_to_remove.len(),
            name,
            preserved
        );
    }
}

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
    clear_programmatic_assets::<Mesh>(world, verbose, "meshes");
    clear_programmatic_assets::<Image>(world, verbose, "images");
    clear_programmatic_assets::<StandardMaterial>(world, verbose, "materials");
    clear_programmatic_assets::<AnimationClip>(world, verbose, "animation clips");
    clear_programmatic_assets::<AnimationGraph>(world, verbose, "animation graphs");
    clear_programmatic_assets::<Scene>(world, verbose, "scenes");

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
