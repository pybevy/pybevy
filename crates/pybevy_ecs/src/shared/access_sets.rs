use bevy::ecs::{
    component::ComponentId,
    query::{FilteredAccess, FilteredAccessSet},
};

/// Build a `FilteredAccessSet` from component and resource access information.
///
/// Constructs the Bevy scheduler metadata from resolved ComponentIds.
/// Tells Bevy's scheduler what components this system reads, writes, and
/// filters on, as well as `Res<T>`, `ResMut<T>`, and `Assets<T>` resource
/// accesses, enabling parallel scheduling and preventing cross-system data races.
pub fn build_full_access_set(
    components_to_read: &[ComponentId],
    components_to_write: &[ComponentId],
    with_filters: &[ComponentId],
    resources_to_read: &[ComponentId],
    resources_to_write: &[ComponentId],
) -> FilteredAccessSet {
    let mut set = FilteredAccessSet::default();
    let mut access = FilteredAccess::default();

    for &id in components_to_write {
        access.add_write(id);
    }
    for &id in components_to_read {
        access.add_read(id);
    }
    for &id in with_filters {
        access.and_with(id);
    }
    for &id in resources_to_write {
        access.add_write(id);
    }
    for &id in resources_to_read {
        access.add_read(id);
    }

    set.add(access);
    set
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{component::Component, world::World};

    use super::*;

    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;
    #[derive(Component)]
    struct C;

    fn ids(world: &mut World) -> (ComponentId, ComponentId, ComponentId) {
        (
            world.register_component::<A>(),
            world.register_component::<B>(),
            world.register_component::<C>(),
        )
    }

    #[test]
    fn empty_inputs_produces_empty_set() {
        let set = build_full_access_set(&[], &[], &[], &[], &[]);
        assert!(!set.combined_access().has_any_read());
        assert!(!set.combined_access().has_any_write());
    }

    #[test]
    fn read_components_registered() {
        let mut world = World::new();
        let (a, b, _) = ids(&mut world);

        let set = build_full_access_set(&[a, b], &[], &[], &[], &[]);
        let combined = set.combined_access();
        assert!(combined.has_read(a));
        assert!(combined.has_read(b));
        assert!(!combined.has_write(a));
    }

    #[test]
    fn write_components_registered() {
        let mut world = World::new();
        let (a, _, _) = ids(&mut world);

        let set = build_full_access_set(&[], &[a], &[], &[], &[]);
        let combined = set.combined_access();
        assert!(combined.has_write(a));
    }

    #[test]
    fn read_and_write_together() {
        let mut world = World::new();
        let (a, b, _) = ids(&mut world);

        let set = build_full_access_set(&[a], &[b], &[], &[], &[]);
        let combined = set.combined_access();
        assert!(combined.has_read(a));
        assert!(combined.has_write(b));
        assert!(!combined.has_write(a));
    }

    #[test]
    fn unrelated_component_not_registered() {
        let mut world = World::new();
        let (a, b, _) = ids(&mut world);

        let set = build_full_access_set(&[a], &[], &[], &[], &[]);
        let combined = set.combined_access();
        assert!(!combined.has_read(b));
    }

    #[test]
    fn full_access_set_all_fields() {
        let mut world = World::new();
        let (a, b, c) = ids(&mut world);

        // a: component read + resource write, b: component write, c: resource read.
        // Resources are components, so reads/writes are tracked through the unified
        // component-access API regardless of component vs resource origin.
        let set = build_full_access_set(&[a], &[b], &[], &[c], &[a]);
        let combined = set.combined_access();
        assert!(combined.has_read(a)); // a: component read
        assert!(combined.has_write(a)); // a: resource write
        assert!(combined.has_write(b)); // b: component write
        assert!(combined.has_read(c)); // c: resource read
        assert!(!combined.has_write(c)); // c is read-only (no write access)
    }

    #[test]
    fn resource_read_registered() {
        let mut world = World::new();
        let (a, _, _) = ids(&mut world);

        let set = build_full_access_set(&[], &[], &[], &[a], &[]);
        let combined = set.combined_access();
        assert!(combined.has_read(a));
        assert!(!combined.has_write(a));
    }

    #[test]
    fn resource_write_registered() {
        let mut world = World::new();
        let (a, _, _) = ids(&mut world);

        let set = build_full_access_set(&[], &[], &[], &[], &[a]);
        let combined = set.combined_access();
        assert!(combined.has_write(a));
    }

    #[test]
    fn resource_and_component_together() {
        let mut world = World::new();
        let (a, b, _) = ids(&mut world);

        let set = build_full_access_set(&[a], &[], &[], &[], &[b]);
        let combined = set.combined_access();
        assert!(combined.has_read(a));
        assert!(combined.has_write(b));
    }

    #[test]
    fn two_resource_write_sets_conflict() {
        let mut world = World::new();
        let (a, _, _) = ids(&mut world);

        let set1 = build_full_access_set(&[], &[], &[], &[], &[a]);
        let set2 = build_full_access_set(&[], &[], &[], &[], &[a]);

        // Combined access of set1 should conflict with set2's access
        assert!(!set1.is_compatible(&set2));
    }

    #[test]
    fn resource_read_and_write_conflict() {
        let mut world = World::new();
        let (a, _, _) = ids(&mut world);

        let set1 = build_full_access_set(&[], &[], &[], &[a], &[]);
        let set2 = build_full_access_set(&[], &[], &[], &[], &[a]);

        assert!(!set1.is_compatible(&set2));
    }

    #[test]
    fn resource_reads_compatible() {
        let mut world = World::new();
        let (a, _, _) = ids(&mut world);

        let set1 = build_full_access_set(&[], &[], &[], &[a], &[]);
        let set2 = build_full_access_set(&[], &[], &[], &[a], &[]);

        assert!(set1.is_compatible(&set2));
    }

    #[test]
    fn different_resources_write_compatible() {
        let mut world = World::new();
        let (a, b, _) = ids(&mut world);

        let set1 = build_full_access_set(&[], &[], &[], &[], &[a]);
        let set2 = build_full_access_set(&[], &[], &[], &[], &[b]);

        assert!(set1.is_compatible(&set2));
    }
}
