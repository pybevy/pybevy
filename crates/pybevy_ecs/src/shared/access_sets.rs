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

    set.add(access);
    set
}
