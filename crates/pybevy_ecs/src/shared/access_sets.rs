use bevy::ecs::{
    component::ComponentId,
    query::{FilteredAccess, FilteredAccessSet},
};

/// Build a `FilteredAccessSet` from collected component access information.
///
/// Constructs the Bevy scheduler metadata from resolved ComponentIds.
/// Tells Bevy's scheduler what components this system reads, writes, and
/// filters on, enabling parallel scheduling.
pub fn build_access_set(
    components_to_read: &[ComponentId],
    components_to_write: &[ComponentId],
    with_filters: &[ComponentId],
) -> FilteredAccessSet {
    build_full_access_set(
        components_to_read,
        components_to_write,
        with_filters,
        &[],
        &[],
    )
}

/// Build a `FilteredAccessSet` from component AND resource access information.
///
/// Extends `build_access_set` with resource read/write tracking, telling Bevy's
/// scheduler about `Res<T>`, `ResMut<T>`, and `Assets<T>` accesses so it can
/// prevent cross-system data races.
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
        access.add_component_write(id);
    }
    for &id in components_to_read {
        access.add_component_read(id);
    }
    for &id in with_filters {
        access.and_with(id);
    }
    for &id in resources_to_write {
        access.add_resource_write(id);
    }
    for &id in resources_to_read {
        access.add_resource_read(id);
    }

    set.add(access);
    set
}
