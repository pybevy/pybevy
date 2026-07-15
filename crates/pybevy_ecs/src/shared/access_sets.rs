use bevy::ecs::{component::ComponentId, query::FilteredAccess};

/// Access declared by a single `Query` or `View` parameter, resolved to ComponentIds.
///
/// Each parameter becomes one `FilteredAccess` (via [`QueryParamAccess::build`])
/// that is added to the system's `FilteredAccessSet`. A `FilteredAccess`'s
/// `With`/`Without` filters apply to its whole access, so a filter may only ever
/// narrow archetypes that the runtime query genuinely refuses to match. Declaring
/// a filter the runtime does not enforce fabricates disjointness and lets the
/// scheduler run conflicting systems in parallel.
///
/// Optional components are held in separate lists because their `With` filter must
/// be omitted: an optional component may be absent while the parameter's other
/// accesses still match, so it does not restrict which archetypes are touched.
#[derive(Default)]
pub struct QueryParamAccess {
    /// Required immutable components: read access plus a `With` filter.
    pub reads: Vec<ComponentId>,
    /// Required mutable components: write access plus a `With` filter.
    pub writes: Vec<ComponentId>,
    /// Optional immutable components: read access, no `With` filter.
    pub optional_reads: Vec<ComponentId>,
    /// Optional mutable components: write access, no `With` filter.
    pub optional_writes: Vec<ComponentId>,
    /// `With<T>` filter components.
    pub with: Vec<ComponentId>,
    /// `Without<T>` filter components.
    pub without: Vec<ComponentId>,
}

impl QueryParamAccess {
    /// Build the `FilteredAccess` for this parameter.
    pub fn build(&self) -> FilteredAccess {
        let mut access = FilteredAccess::default();

        // Required components. `FilteredAccess::add_read`/`add_write` also add the
        // matching `With` filter, because the runtime query requires the component
        // present for the access to be reachable.
        for &id in &self.writes {
            access.add_write(id);
        }
        for &id in &self.reads {
            access.add_read(id);
        }

        // Optional components. Register access on the raw `Access` only, so no
        // `With` filter is added: the query still matches archetypes that lack them.
        for &id in &self.optional_writes {
            access.access_mut().add_write(id);
        }
        for &id in &self.optional_reads {
            access.access_mut().add_read(id);
        }

        for &id in &self.with {
            access.and_with(id);
        }
        for &id in &self.without {
            access.and_without(id);
        }

        access
    }
}

/// Build the shared `FilteredAccess` for a system's resource-like access.
///
/// Covers `Res`/`ResMut`, `Assets`, `MessageReader`/`MessageWriter`/`MessageMutator`, and the
/// HotReloadGeneration read. These carry no archetype filters; conflict detection
/// relies purely on the read/write sets. `FilteredAccess::add_read`/`add_write`
/// also add a `With` on the resource's id, which is harmless: two accesses to the
/// same resource share that filter and never rule each other out, and resource ids
/// never intersect a query's component access, so no false disjointness arises.
pub fn build_resource_access(
    resources_to_read: &[ComponentId],
    resources_to_write: &[ComponentId],
) -> FilteredAccess {
    let mut access = FilteredAccess::default();
    for &id in resources_to_write {
        access.add_write(id);
    }
    for &id in resources_to_read {
        access.add_read(id);
    }
    access
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{
        component::Component,
        query::{Added, FilteredAccessSet, With, Without},
        world::World,
    };

    use super::*;

    #[derive(Component)]
    struct T;
    #[derive(Component)]
    struct U;
    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;

    /// Wrap a single `FilteredAccess` in a fresh set.
    fn set_of(access: FilteredAccess) -> FilteredAccessSet {
        let mut set = FilteredAccessSet::default();
        set.add(access);
        set
    }

    /// Wrap several per-parameter `FilteredAccess`es in one set, as a multi-query
    /// system would.
    fn set_of_many(accesses: Vec<FilteredAccess>) -> FilteredAccessSet {
        let mut set = FilteredAccessSet::default();
        for access in accesses {
            set.add(access);
        }
        set
    }

    fn read_id(id: ComponentId) -> QueryParamAccess {
        QueryParamAccess {
            reads: vec![id],
            ..Default::default()
        }
    }

    fn write_id(id: ComponentId) -> QueryParamAccess {
        QueryParamAccess {
            writes: vec![id],
            ..Default::default()
        }
    }

    #[test]
    fn empty_resource_access_is_empty() {
        let access = build_resource_access(&[], &[]);
        assert!(!access.access().has_any_read());
        assert!(!access.access().has_any_write());
    }

    #[test]
    fn resource_read_and_write_registered() {
        let mut world = World::new();
        let a = world.register_component::<A>();
        let b = world.register_component::<B>();

        let access = build_resource_access(&[a], &[b]);
        assert!(access.access().has_read(a));
        assert!(access.access().has_write(b));
        assert!(!access.access().has_write(a));
    }

    #[test]
    fn two_resource_writes_conflict() {
        let mut world = World::new();
        let a = world.register_component::<A>();

        let set1 = set_of(build_resource_access(&[], &[a]));
        let set2 = set_of(build_resource_access(&[], &[a]));
        assert!(!set1.is_compatible(&set2));
    }

    #[test]
    fn resource_read_and_write_conflict() {
        let mut world = World::new();
        let a = world.register_component::<A>();

        let set1 = set_of(build_resource_access(&[a], &[]));
        let set2 = set_of(build_resource_access(&[], &[a]));
        assert!(!set1.is_compatible(&set2));
    }

    #[test]
    fn resource_reads_compatible() {
        let mut world = World::new();
        let a = world.register_component::<A>();

        let set1 = set_of(build_resource_access(&[a], &[]));
        let set2 = set_of(build_resource_access(&[a], &[]));
        assert!(set1.is_compatible(&set2));
    }

    #[test]
    fn different_resource_writes_compatible() {
        let mut world = World::new();
        let a = world.register_component::<A>();
        let b = world.register_component::<B>();

        let set1 = set_of(build_resource_access(&[], &[a]));
        let set2 = set_of(build_resource_access(&[], &[b]));
        assert!(set1.is_compatible(&set2));
    }

    /// Defect 1: `Without` was declared as `With`, fabricating disjointness.
    /// pybevy `Query[Mut[T], Without[A]]` must conflict with native
    /// `Query<&mut T, Without<A>>` (both mutate the same T in the same archetypes).
    #[test]
    fn without_declared_as_without_not_with() {
        let mut world = World::new();
        let t = world.register_component::<T>();
        let a = world.register_component::<A>();

        let pybevy = set_of(
            QueryParamAccess {
                writes: vec![t],
                without: vec![a],
                ..Default::default()
            }
            .build(),
        );

        let native = world.query_filtered::<&mut T, Without<A>>();
        let native = set_of(native.component_access().clone());

        assert!(!pybevy.is_compatible(&native));
    }

    /// Guards against an over-conservative fix: `Query[Mut[T], With[A]]` and native
    /// `Query<&mut T, Without<A>>` are genuinely disjoint and must stay parallel.
    #[test]
    fn with_vs_native_without_stays_compatible() {
        let mut world = World::new();
        let t = world.register_component::<T>();
        let a = world.register_component::<A>();

        let pybevy = set_of(
            QueryParamAccess {
                writes: vec![t],
                with: vec![a],
                ..Default::default()
            }
            .build(),
        );

        let native = world.query_filtered::<&mut T, Without<A>>();
        let native = set_of(native.component_access().clone());

        assert!(pybevy.is_compatible(&native));
    }

    /// Defect 2: `Changed<B>` reads B's change ticks, so it must declare read B.
    /// The combined access must expose the read, and the system must conflict with
    /// a native writer of B (writing B also writes B's ticks).
    #[test]
    fn changed_declares_read_and_conflicts_with_writer() {
        let mut world = World::new();
        let a = world.register_component::<A>();
        let b = world.register_component::<B>();

        // Query[Mut[A], Changed[B]]: write A, and Changed[B] contributes read B.
        let pybevy = set_of(
            QueryParamAccess {
                writes: vec![a],
                reads: vec![b],
                with: vec![b],
                ..Default::default()
            }
            .build(),
        );
        assert!(pybevy.combined_access().has_read(b));

        let native = world.query_filtered::<&mut B, ()>();
        let native = set_of(native.component_access().clone());

        assert!(!pybevy.is_compatible(&native));
    }

    /// Defect 3: `Has<A>` matches regardless of A, so it must add no `With`.
    /// pybevy `Query[Mut[T], Has[A]]` must conflict with native
    /// `Query<&mut T, Without<A>>`.
    #[test]
    fn has_contributes_no_with_filter() {
        let mut world = World::new();
        let t = world.register_component::<T>();

        // Has[A] registers A's id at runtime but contributes no access, no filter.
        // The native query registers A itself through its `Without<A>` filter.
        let pybevy = set_of(write_id(t).build());

        let native = world.query_filtered::<&mut T, Without<A>>();
        let native = set_of(native.component_access().clone());

        assert!(!pybevy.is_compatible(&native));
    }

    /// Defect 3: `AnyOf[A, B]` has OR semantics that conjunctive `With` cannot
    /// express, so it adds no filter. pybevy `Query[Mut[T], AnyOf[A, B]]` must
    /// conflict with native `Query<&mut T, (With<A>, Without<B>)>`.
    #[test]
    fn anyof_contributes_no_with_filter() {
        let mut world = World::new();
        let t = world.register_component::<T>();

        // AnyOf[A, B] registers A and B ids at runtime but contributes no filter.
        // The native query registers them through its `With<A>`/`Without<B>` filters.
        let pybevy = set_of(write_id(t).build());

        let native = world.query_filtered::<&mut T, (With<A>, Without<B>)>();
        let native = set_of(native.component_access().clone());

        assert!(!pybevy.is_compatible(&native));
    }

    /// Defect 4: filters are per-parameter, so a `With[A]` on one query must not
    /// narrow another query's access. A two-query system {`Query[Mut[T]]`,
    /// `Query[Mut[U], With[A]]`} must conflict with native `Query<&mut T, Without<A>>`.
    #[test]
    fn per_parameter_filters_do_not_narrow_siblings() {
        let mut world = World::new();
        let t = world.register_component::<T>();
        let u = world.register_component::<U>();
        let a = world.register_component::<A>();

        let pybevy = set_of_many(vec![
            write_id(t).build(),
            QueryParamAccess {
                writes: vec![u],
                with: vec![a],
                ..Default::default()
            }
            .build(),
        ]);

        let native = world.query_filtered::<&mut T, Without<A>>();
        let native = set_of(native.component_access().clone());

        assert!(!pybevy.is_compatible(&native));
    }

    /// An optional component must not add a `With` filter. pybevy
    /// `Query[Mut[U], Option[T]]` must conflict with native
    /// `Query<&mut U, Without<T>>`; it would wrongly be compatible if optional T
    /// contributed `and_with(T)`.
    #[test]
    fn optional_component_adds_no_with_filter() {
        let mut world = World::new();
        let u = world.register_component::<U>();
        let t = world.register_component::<T>();

        let pybevy = set_of(
            QueryParamAccess {
                writes: vec![u],
                optional_reads: vec![t],
                ..Default::default()
            }
            .build(),
        );

        let native = world.query_filtered::<&mut U, Without<T>>();
        let native = set_of(native.component_access().clone());

        assert!(!pybevy.is_compatible(&native));
    }

    /// View coverage: a View with a `Changed[B]` filter declares read B and must
    /// conflict with a native writer of B.
    #[test]
    fn view_changed_filter_declares_read() {
        let mut world = World::new();
        let a = world.register_component::<A>();
        let b = world.register_component::<B>();

        // View[Mut[A]] with Changed[B]: write A, read B (with A and B).
        let pybevy = set_of(
            QueryParamAccess {
                writes: vec![a],
                reads: vec![b],
                with: vec![b],
                ..Default::default()
            }
            .build(),
        );
        assert!(pybevy.combined_access().has_read(b));

        let native = world.query_filtered::<&mut B, ()>();
        let native = set_of(native.component_access().clone());

        assert!(!pybevy.is_compatible(&native));
    }

    /// Two read-only queries over the same component stay parallel.
    #[test]
    fn shared_reads_stay_compatible() {
        let mut world = World::new();
        let t = world.register_component::<T>();

        let set1 = set_of(read_id(t).build());
        let set2 = set_of(read_id(t).build());
        assert!(set1.is_compatible(&set2));
    }

    /// `Added` behaves like `Changed` for access: it declares a read.
    #[test]
    fn added_matches_native_added_access() {
        let mut world = World::new();
        let a = world.register_component::<A>();
        let b = world.register_component::<B>();

        // Query[Mut[A], Added[B]]: write A, read B (with A and B).
        let pybevy = set_of(
            QueryParamAccess {
                writes: vec![a],
                reads: vec![b],
                with: vec![b],
                ..Default::default()
            }
            .build(),
        );

        let native = world.query_filtered::<&mut A, Added<B>>();
        let native_set = set_of(native.component_access().clone());
        // Same declared shape, so a second writer of A conflicts with both.
        let writer = world.query_filtered::<&mut A, ()>();
        let writer = set_of(writer.component_access().clone());

        assert!(!pybevy.is_compatible(&writer));
        assert!(!native_set.is_compatible(&writer));
    }
}
