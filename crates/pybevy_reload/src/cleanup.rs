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
