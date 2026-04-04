use bevy::ecs::{
    component::ComponentId,
    query::{QueryBuilder, QueryState},
    world::{FilteredEntityMut, World},
};

/// Specification for building a Bevy QueryState.
///
/// Collects resolved ComponentIds so the QueryState can be constructed with
/// Bevy's native archetype-indexed matching.
pub struct QueryBuildSpec {
    /// Components to query: (ComponentId, is_optional)
    pub components: Vec<(ComponentId, bool)>,
    /// With filter component IDs
    pub with_filters: Vec<ComponentId>,
    /// Without filter component IDs
    pub without_filters: Vec<ComponentId>,
    /// Changed filter component IDs (registers ref access for change tracking)
    pub changed_filters: Vec<ComponentId>,
    /// Added filter component IDs (registers ref access for added tracking)
    pub added_filters: Vec<ComponentId>,
    /// AnyOf filter component IDs (Or<(With<A>, With<B>, ...)>)
    pub anyof_filters: Vec<ComponentId>,
}

/// Build a Bevy `QueryState<FilteredEntityMut>` from a specification.
///
/// Callers resolve their parameter types to `ComponentId`s, then call this
/// function to build the QueryState.
pub fn build_query_state<'a>(
    world: &'a mut World,
    spec: &QueryBuildSpec,
) -> QueryState<FilteredEntityMut<'a, 'a>> {
    let mut builder = QueryBuilder::<FilteredEntityMut>::new(world);

    for &(id, optional) in &spec.components {
        if optional {
            builder.optional(|b| {
                b.mut_id(id);
            });
        } else {
            builder.mut_id(id);
        }
    }

    for &id in &spec.with_filters {
        builder.with_id(id);
    }

    for &id in &spec.without_filters {
        builder.without_id(id);
    }

    for &id in &spec.changed_filters {
        builder.ref_id(id);
    }

    for &id in &spec.added_filters {
        builder.ref_id(id);
    }

    if !spec.anyof_filters.is_empty() {
        builder.or(|b| {
            for &id in &spec.anyof_filters {
                b.with_id(id);
            }
        });
    }

    builder.build()
}
