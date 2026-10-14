use bevy::{
    ecs::world::World,
    prelude::{ChildOf, Children, Entity, GlobalTransform, Transform, Without},
};

/// Propagate transforms through the full hierarchy after mutations.
/// Updates GlobalTransform for root entities first, then recursively for children.
/// This ensures spatial queries see up-to-date world positions within the same frame.
pub(crate) fn propagate_transforms(world: &mut World) {
    // Update root entities (no ChildOf)
    let mut root_query = world.query_filtered::<(Entity, &Transform), Without<ChildOf>>();
    let roots: Vec<(Entity, GlobalTransform)> = root_query
        .iter(world)
        .map(|(e, t)| (e, GlobalTransform::from(*t)))
        .collect();
    for (entity, gt) in &roots {
        if let Some(mut gt_mut) = world.get_mut::<GlobalTransform>(*entity) {
            *gt_mut = *gt;
        }
    }

    // Propagate to children (breadth-first)
    let mut queue: Vec<(Entity, GlobalTransform)> = roots;
    while !queue.is_empty() {
        let mut next_queue = Vec::new();
        for (parent_entity, parent_gt) in &queue {
            // Get children of this entity
            let child_entities: Vec<Entity> =
                if let Some(children) = world.get::<Children>(*parent_entity) {
                    children.iter().copied().collect()
                } else {
                    continue;
                };
            for child in child_entities {
                if let Some(child_transform) = world.get::<Transform>(child) {
                    let child_gt = parent_gt.mul_transform(*child_transform);
                    if let Some(mut gt_mut) = world.get_mut::<GlobalTransform>(child) {
                        *gt_mut = child_gt;
                    }
                    next_queue.push((child, child_gt));
                }
            }
        }
        queue = next_queue;
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec3;

    use super::*;

    #[test]
    fn propagate_transforms_updates_root() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();

        // Modify Transform without running propagation
        world.get_mut::<Transform>(entity).unwrap().translation = Vec3::new(10.0, 20.0, 30.0);

        // GlobalTransform should still be at origin
        let gt = world.get::<GlobalTransform>(entity).unwrap();
        assert_eq!(gt.translation(), Vec3::ZERO);

        // Propagate
        propagate_transforms(&mut world);

        // Now GlobalTransform should match
        let gt = world.get::<GlobalTransform>(entity).unwrap();
        assert_eq!(gt.translation(), Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn propagate_transforms_updates_children() {
        let mut world = World::new();

        // Parent at x=100
        let parent = world
            .spawn((
                Transform::from_xyz(100.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();

        // Child at local x=40
        let child = world
            .spawn((
                Transform::from_xyz(40.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();

        // Establish hierarchy
        world.entity_mut(parent).add_children(&[child]);

        // Move parent to x=140
        world.get_mut::<Transform>(parent).unwrap().translation.x = 140.0;

        // Before propagation, child GlobalTransform is stale
        let gt = world.get::<GlobalTransform>(child).unwrap();
        assert_eq!(gt.translation().x, 0.0);

        // Propagate
        propagate_transforms(&mut world);

        // Parent should be at x=140
        let gt = world.get::<GlobalTransform>(parent).unwrap();
        assert_eq!(gt.translation().x, 140.0);

        // Child should be at x=140+40=180
        let gt = world.get::<GlobalTransform>(child).unwrap();
        assert_eq!(gt.translation().x, 180.0);
    }

    #[test]
    fn propagate_transforms_follows_a_detach() {
        let mut world = World::new();
        let parent = world
            .spawn((
                Transform::from_xyz(100.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();
        let child = world
            .spawn((
                Transform::from_xyz(40.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();
        world.entity_mut(parent).add_children(&[child]);
        propagate_transforms(&mut world);
        assert_eq!(
            world.get::<GlobalTransform>(child).unwrap().translation().x,
            140.0
        );

        world.entity_mut(child).remove::<ChildOf>();
        propagate_transforms(&mut world);

        assert_eq!(
            world.get::<GlobalTransform>(child).unwrap().translation().x,
            40.0
        );
    }
}
