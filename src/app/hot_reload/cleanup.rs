use bevy::ecs::{entity::Entity, prelude::Without, resource::IsResource, world::World};
use pybevy_reload::is_verbose;
use pyo3::prelude::*;

use super::bindings::PyHotReloadControl;
use crate::ecs::resource_type::{PyResourceStorage, PyResourceType};

/// Clear all custom Python resources from PyResourceStorage
/// Preserves built-in resources (Time, AssetServer, etc.) and HotReloadControl
pub(crate) fn clear_custom_resources(world: &mut World, verbose: bool) {
    // Save HotReloadControl before clearing (it needs to persist across reloads)
    // We iterate through all resources and save the one that is HotReloadControl
    let hot_reload_control = Python::attach(|py| {
        if let Some(storage) = world.get_resource::<PyResourceStorage>() {
            // Iterate through all resources to find HotReloadControl
            for resource in storage.resources.values() {
                let resource_bound = resource.bind(py);
                // Check if this resource is a HotReloadControl by trying to extract it
                if resource_bound
                    .extract::<PyRef<PyHotReloadControl>>()
                    .is_ok()
                {
                    return Some(resource.clone_ref(py));
                }
            }
        }
        None
    });

    // Clear PyResourceStorage (custom Python resources)
    if let Some(mut storage) = world.get_resource_mut::<PyResourceStorage>() {
        let count = storage.resources.len();

        // Drop all Python resource instances within Python::attach to ensure GIL
        Python::attach(|_py| {
            storage.resources.clear();
        });

        if verbose {
            eprintln!("   → Cleared {} custom Python resources", count);
        }
    }

    // Restore HotReloadControl after clearing
    if let Some(control) = hot_reload_control {
        Python::attach(|py| {
            // Get the type from the instance
            let control_bound = control.bind(py);
            let control_type = control_bound.get_type();
            let type_ptr = control_type.as_type_ptr();
            let py_resource_type = PyResourceType::Custom(type_ptr);

            // Re-insert the saved control
            if let Err(e) = py_resource_type.insert_into_world(world, py, control) {
                eprintln!("Warning: Failed to restore HotReloadControl: {:?}", e);
            }
        });
    }
}

/// Clear all entities and custom resources for a complete scene reset.
///
/// Similar to hot reload's Full mode entity/resource clearing but without
/// the system reloading logic. Used by `App.clear_scene()` to reset state.
///
/// Clears:
/// - All entities (clears everything not in base set)
/// - Custom Python resources from PyResourceStorage
///
/// Preserves:
/// - Built-in Bevy resources (Time, AssetServer, etc.)
/// - RenderDevice and render infrastructure
/// - Plugin state
pub fn clear_entities_and_resources(world: &mut World) {
    // Despawn ALL scene entities (complete clean slate)
    let all_entities: Vec<Entity> = world
        .query_filtered::<Entity, Without<IsResource>>()
        .iter(world)
        .collect();

    if is_verbose() {
        eprintln!("[clear_scene] Despawning {} entities", all_entities.len());
    }

    for entity in all_entities {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    // Clear custom Python resources
    if is_verbose() {
        eprintln!("[clear_scene] Clearing custom resources");
    }
    clear_custom_resources(world, is_verbose());

    if is_verbose() {
        eprintln!("[clear_scene] Scene cleared successfully");
    }
}
