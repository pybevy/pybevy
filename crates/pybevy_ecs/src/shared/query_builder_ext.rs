use bevy::ecs::{
    component::ComponentId,
    query::{QueryBuilder, QueryData, QueryState},
    world::{FilteredEntityMut, FilteredEntityRef, World},
};

/// A single component entry in a query specification.
#[derive(Clone)]
pub struct QueryComponent {
    pub id: ComponentId,
    pub optional: bool,
    pub mutable: bool,
}

/// Specification for building a Bevy QueryState.
///
/// Collects resolved ComponentIds so the QueryState can be constructed with
/// Bevy's native archetype-indexed matching.
pub struct QueryBuildSpec {
    pub components: Vec<QueryComponent>,
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

impl QueryBuildSpec {
    /// Returns true if all queried components are read-only (no mutable access requested).
    pub fn is_read_only(&self) -> bool {
        !self.components.iter().any(|c| c.mutable)
    }
}

/// Apply with/without/changed/added/anyof filters to any QueryBuilder.
fn apply_filters<D: QueryData>(builder: &mut QueryBuilder<D>, spec: &QueryBuildSpec) {
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

    for comp in &spec.components {
        if comp.optional {
            builder.optional(|b| {
                if comp.mutable {
                    b.mut_id(comp.id);
                } else {
                    b.ref_id(comp.id);
                }
            });
        } else if comp.mutable {
            builder.mut_id(comp.id);
        } else {
            builder.ref_id(comp.id);
        }
    }

    apply_filters(&mut builder, spec);
    builder.build()
}

/// Build a Bevy `QueryState<FilteredEntityRef>` from a read-only specification.
///
/// This should only be called when `spec.is_read_only()` returns true.
/// Uses `FilteredEntityRef` to signal to Bevy's parallel executor that
/// no write access is held, enabling concurrent scheduling.
pub fn build_query_state_ref<'a>(
    world: &'a mut World,
    spec: &QueryBuildSpec,
) -> QueryState<FilteredEntityRef<'a, 'a>> {
    let mut builder = QueryBuilder::<FilteredEntityRef>::new(world);

    for comp in &spec.components {
        debug_assert!(!comp.mutable, "build_query_state_ref called with mutable component");
        if comp.optional {
            builder.optional(|b| {
                b.ref_id(comp.id);
            });
        } else {
            builder.ref_id(comp.id);
        }
    }

    apply_filters(&mut builder, spec);
    builder.build()
}
