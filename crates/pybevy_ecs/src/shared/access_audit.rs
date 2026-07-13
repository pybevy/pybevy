//! Debug-only auditor: verify a system's declared scheduler access covers what
//! its queries actually touch.
//!
//! `build_declared_access` derives each `Query`/`View` parameter's
//! `FilteredAccess` from the ParamSpec through `QueryParamAccess`. That is the
//! set the multithreaded executor uses to decide which systems may run in
//! parallel. The runtime query is built from the same ParamSpec down a separate
//! path (a `QueryBuilder` producing a `QueryState`), and Bevy computes that
//! state's real `component_access()` itself. If the two derivations ever diverge
//! so that a query reads or writes a component the declared set omits, the
//! scheduler can place a conflicting system alongside it and race.
//!
//! This asserts, in debug builds only, that every query's actual access is a
//! subset of the declared set, turning such a divergence into a loud panic at
//! `initialize` instead of a silent data race at runtime. It is the runtime
//! guard complementing the static per-parameter tests in `access_sets` and
//! `param_spec`.

use bevy::ecs::query::{FilteredAccess, FilteredAccessSet};

/// Assert the declared access set covers one query parameter's actual access.
///
/// `declared` is the system's full `FilteredAccessSet` (from
/// `build_declared_access`); `actual` is a query parameter's
/// `QueryState::component_access()`. Panics if the query reads or writes any
/// component access the declared set does not cover.
#[cfg(debug_assertions)]
pub fn assert_query_access_declared(
    system_name: &str,
    query_index: usize,
    declared: &FilteredAccessSet,
    actual: &FilteredAccess,
) {
    if !actual.access().is_subset(declared.combined_access()) {
        panic!(
            "pybevy access-declaration bug in system `{system_name}`: query parameter \
             #{query_index} reads or writes component access that `initialize` did not \
             declare to the scheduler. The declared FilteredAccessSet (derived from the \
             ParamSpec via QueryParamAccess) must cover the QueryState's real \
             component_access(), but does not. Under the multithreaded executor this lets \
             a conflicting system run in parallel and race.\n  \
             declared (combined): {:?}\n  actual (query):      {:?}",
            declared.combined_access(),
            actual.access(),
        );
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use bevy::ecs::{
        component::{Component, ComponentId},
        query::FilteredAccessSet,
        world::World,
    };

    use super::*;
    use crate::shared::access_sets::QueryParamAccess;

    #[derive(Component)]
    struct T;
    #[derive(Component)]
    struct U;

    /// A declared set that reads exactly `ids`, as a single query parameter.
    fn declared_reading(ids: Vec<ComponentId>) -> FilteredAccessSet {
        let mut set = FilteredAccessSet::default();
        set.add(
            QueryParamAccess {
                reads: ids,
                ..Default::default()
            }
            .build(),
        );
        set
    }

    #[test]
    fn covered_query_access_passes() {
        let mut world = World::new();
        let t = world.register_component::<T>();
        let declared = declared_reading(vec![t]);
        // A query that reads T is covered by a declaration of read T.
        let actual = world.query_filtered::<&T, ()>().component_access().clone();
        assert_query_access_declared("sys", 0, &declared, &actual);
    }

    #[test]
    #[should_panic(expected = "access-declaration bug")]
    fn undeclared_component_read_panics() {
        let mut world = World::new();
        let t = world.register_component::<T>();
        let _u = world.register_component::<U>();
        // Declared: read T only. Actual query reads U -> not covered -> panic.
        let declared = declared_reading(vec![t]);
        let actual = world.query_filtered::<&U, ()>().component_access().clone();
        assert_query_access_declared("sys", 0, &declared, &actual);
    }

    #[test]
    #[should_panic(expected = "access-declaration bug")]
    fn write_beyond_declared_read_panics() {
        let mut world = World::new();
        let t = world.register_component::<T>();
        // Declared: read T. Actual: writes T -> a write is not covered by a read.
        let declared = declared_reading(vec![t]);
        let actual = world
            .query_filtered::<&mut T, ()>()
            .component_access()
            .clone();
        assert_query_access_declared("sys", 0, &declared, &actual);
    }
}
