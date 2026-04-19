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

/// Configure a `QueryBuilder<FilteredEntityMut>` from a specification.
///
/// Registers component access (ref or mut, required or optional) and applies
/// all filters. Call `builder.build()` afterwards to obtain the `QueryState`.
pub fn configure_mut_query(
    builder: &mut QueryBuilder<FilteredEntityMut>,
    spec: &QueryBuildSpec,
) {
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

    apply_filters(builder, spec);
}

/// Configure a `QueryBuilder<FilteredEntityRef>` from a read-only specification.
///
/// This should only be called when `spec.is_read_only()` returns true.
pub fn configure_ref_query(
    builder: &mut QueryBuilder<FilteredEntityRef>,
    spec: &QueryBuildSpec,
) {
    for comp in &spec.components {
        debug_assert!(
            !comp.mutable,
            "configure_ref_query called with mutable component"
        );
        if comp.optional {
            builder.optional(|b| {
                b.ref_id(comp.id);
            });
        } else {
            builder.ref_id(comp.id);
        }
    }

    apply_filters(builder, spec);
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
    configure_mut_query(&mut builder, spec);
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
    configure_ref_query(&mut builder, spec);
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::component::Component;

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

    fn spec_with(components: Vec<QueryComponent>) -> QueryBuildSpec {
        QueryBuildSpec {
            components,
            with_filters: vec![],
            without_filters: vec![],
            changed_filters: vec![],
            added_filters: vec![],
            anyof_filters: vec![],
        }
    }

    #[test]
    fn is_read_only_empty() {
        let spec = spec_with(vec![]);
        assert!(spec.is_read_only());
    }

    #[test]
    fn is_read_only_all_refs() {
        let mut world = World::new();
        let (a, b, _) = ids(&mut world);
        let spec = spec_with(vec![
            QueryComponent { id: a, optional: false, mutable: false },
            QueryComponent { id: b, optional: false, mutable: false },
        ]);
        assert!(spec.is_read_only());
    }

    #[test]
    fn is_read_only_one_mutable() {
        let mut world = World::new();
        let (a, b, _) = ids(&mut world);
        let spec = spec_with(vec![
            QueryComponent { id: a, optional: false, mutable: false },
            QueryComponent { id: b, optional: false, mutable: true },
        ]);
        assert!(!spec.is_read_only());
    }

    #[test]
    fn build_query_matches_entities() {
        let mut world = World::new();
        let (a_id, _, _) = ids(&mut world);

        world.spawn(A);
        world.spawn(B); // should not match
        world.spawn((A, B));

        let spec = spec_with(vec![
            QueryComponent { id: a_id, optional: false, mutable: false },
        ]);
        let mut builder = QueryBuilder::<FilteredEntityRef>::new(&mut world);
        configure_ref_query(&mut builder, &spec);
        let mut state = builder.build();
        assert_eq!(state.iter(&world).count(), 2);
    }

    #[test]
    fn build_query_with_filter() {
        let mut world = World::new();
        let (a_id, b_id, _) = ids(&mut world);

        world.spawn(A);
        world.spawn((A, B));

        let spec = QueryBuildSpec {
            components: vec![
                QueryComponent { id: a_id, optional: false, mutable: false },
            ],
            with_filters: vec![b_id],
            without_filters: vec![],
            changed_filters: vec![],
            added_filters: vec![],
            anyof_filters: vec![],
        };
        let mut builder = QueryBuilder::<FilteredEntityRef>::new(&mut world);
        configure_ref_query(&mut builder, &spec);
        let mut state = builder.build();
        assert_eq!(state.iter(&world).count(), 1);
    }

    #[test]
    fn build_query_without_filter() {
        let mut world = World::new();
        let (a_id, b_id, _) = ids(&mut world);

        world.spawn(A);
        world.spawn((A, B));

        let spec = QueryBuildSpec {
            components: vec![
                QueryComponent { id: a_id, optional: false, mutable: false },
            ],
            with_filters: vec![],
            without_filters: vec![b_id],
            changed_filters: vec![],
            added_filters: vec![],
            anyof_filters: vec![],
        };
        let mut builder = QueryBuilder::<FilteredEntityRef>::new(&mut world);
        configure_ref_query(&mut builder, &spec);
        let mut state = builder.build();
        assert_eq!(state.iter(&world).count(), 1);
    }

    #[test]
    fn build_query_optional_component() {
        let mut world = World::new();
        let (a_id, b_id, _) = ids(&mut world);

        world.spawn(A);
        world.spawn((A, B));

        // A required, B optional - should match both entities
        let spec = spec_with(vec![
            QueryComponent { id: a_id, optional: false, mutable: false },
            QueryComponent { id: b_id, optional: true, mutable: false },
        ]);
        let mut builder = QueryBuilder::<FilteredEntityRef>::new(&mut world);
        configure_ref_query(&mut builder, &spec);
        let mut state = builder.build();
        assert_eq!(state.iter(&world).count(), 2);
    }

    #[test]
    fn build_query_mutable_access() {
        let mut world = World::new();
        let (a_id, _, _) = ids(&mut world);

        world.spawn(A);

        let spec = spec_with(vec![
            QueryComponent { id: a_id, optional: false, mutable: true },
        ]);
        let mut builder = QueryBuilder::<FilteredEntityMut>::new(&mut world);
        configure_mut_query(&mut builder, &spec);
        let mut state = builder.build();
        assert_eq!(state.iter_mut(&mut world).count(), 1);
    }

    #[test]
    fn build_query_anyof_filter() {
        let mut world = World::new();
        let (a_id, b_id, c_id) = ids(&mut world);

        world.spawn(A);         // has A but no B or C -> no match
        world.spawn((A, B));    // has B -> matches
        world.spawn((A, C));    // has C -> matches
        world.spawn(B);         // no A -> no match

        let spec = QueryBuildSpec {
            components: vec![
                QueryComponent { id: a_id, optional: false, mutable: false },
            ],
            with_filters: vec![],
            without_filters: vec![],
            changed_filters: vec![],
            added_filters: vec![],
            anyof_filters: vec![b_id, c_id],
        };
        let mut builder = QueryBuilder::<FilteredEntityRef>::new(&mut world);
        configure_ref_query(&mut builder, &spec);
        let mut state = builder.build();
        assert_eq!(state.iter(&world).count(), 2);
    }
}
