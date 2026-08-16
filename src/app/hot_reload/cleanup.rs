use bevy::ecs::{entity::Entity, prelude::Without, resource::IsResource, world::World};
use pybevy_core::CustomResourceInfo;
use pybevy_reload::is_verbose;
use pyo3::prelude::*;

use super::bindings::PyHotReloadControl;
use crate::ecs::resource_type::register_custom_resource;

/// Clear custom Python resource value components while preserving HotReloadControl.
pub(crate) fn clear_custom_resources(world: &mut World, verbose: bool) {
    let custom_entries = world
        .get_resource::<CustomResourceInfo>()
        .map(|info| {
            info.iter()
                .map(|(id, entry)| (id, entry.type_ptr))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let (resource_ids, control) = Python::attach(|py| {
        let mut resource_ids = Vec::new();
        let mut control = None;
        for (id, type_ptr) in custom_entries {
            let is_control = world.get_resource_by_id(id).is_some_and(|value| {
                // SAFETY: CustomResourceInfo entries use the Py<PyAny> resource descriptor.
                let value = unsafe { value.deref::<Py<PyAny>>() };
                value
                    .bind(py)
                    .extract::<PyRef<PyHotReloadControl>>()
                    .is_ok()
            });
            if is_control {
                control = Some(type_ptr);
            } else {
                resource_ids.push(id);
            }
        }
        (resource_ids, control)
    });

    let count = resource_ids
        .into_iter()
        .filter(|id| world.remove_resource_by_id(*id))
        .count();

    if let Some(type_ptr) = control {
        Python::attach(|py| register_custom_resource(world, type_ptr, py));
    }

    if verbose {
        eprintln!("   → Cleared {} custom Python resources", count);
    }
}

/// Clear all entities and custom resources for a complete scene reset.
///
/// Similar to hot reload's Full mode entity/resource clearing but without
/// the system reloading logic. Used by `App.clear_scene()` to reset state.
///
/// Clears:
/// - All entities (clears everything not in base set)
/// - Custom Python resource value components
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
