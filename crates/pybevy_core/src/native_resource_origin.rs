use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
};

use bevy::ecs::{resource::Resource, world::World};

/// Presence before the first successful interpreter insertion or removal.
#[derive(Resource, Default)]
pub struct NativeResourceOrigins(HashMap<TypeId, bool>);

pub fn record_native_resource_origin(world: &mut World, type_id: TypeId, was_present: bool) {
    world
        .get_resource_or_insert_with(NativeResourceOrigins::default)
        .0
        .entry(type_id)
        .or_insert(was_present);
}

/// Separate plugin defaults from values first created by scene code.
pub fn native_resource_baseline(world: &World, mut present: HashSet<TypeId>) -> HashSet<TypeId> {
    if let Some(origins) = world.get_resource::<NativeResourceOrigins>() {
        for (&type_id, &was_present) in &origins.0 {
            if was_present {
                present.insert(type_id);
            } else {
                present.remove(&type_id);
            }
        }
    }
    present
}

#[cfg(test)]
mod tests {
    use std::{any::TypeId, collections::HashSet};

    use bevy::ecs::world::World;

    use super::{native_resource_baseline, record_native_resource_origin};

    #[test]
    fn baseline_keeps_plugin_defaults_and_excludes_scene_created_values() {
        let mut world = World::new();
        let plugin = TypeId::of::<u32>();
        let scene = TypeId::of::<u64>();
        let untouched = TypeId::of::<i32>();
        record_native_resource_origin(&mut world, plugin, true);
        record_native_resource_origin(&mut world, scene, false);
        record_native_resource_origin(&mut world, scene, true);
        record_native_resource_origin(&mut world, plugin, false);
        assert_eq!(
            native_resource_baseline(&world, HashSet::from([scene, untouched])),
            HashSet::from([plugin, untouched])
        );
    }
}
