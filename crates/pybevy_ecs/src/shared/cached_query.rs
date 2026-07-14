//! Shared per-Query cached state: the type-erased `QueryState` plus the
//! per-entity Changed/Added tick check.
//!
//! Both backends build one [`CachedQueryCore`] per Query parameter in
//! `System::initialize` and reuse it across every run. The backend keeps its
//! own extraction plan (how entities become Python objects) next to the core;
//! everything below is interpreter-free.
//!
//! Iterator erasure stays backend-side: producing items requires the
//! backend's entity-access wrapper types, which live outside this crate.

use bevy::ecs::{
    change_detection::{ComponentTicks, Tick},
    component::ComponentId,
    query::{FilteredAccess, QueryState},
    world::{FilteredEntityMut, FilteredEntityRef, World, unsafe_world_cell::UnsafeWorldCell},
};

use super::query_builder_ext::{QueryBuildSpec, build_query_state, build_query_state_ref};

/// Type-erased Bevy QueryState. Owns a heap-allocated QueryState behind a raw
/// pointer; Drop reconstructs the correct Box type to deallocate.
///
/// SAFETY: the erased lifetimes are tied to system execution scope; the caller
/// fences every dereference with its run-validity mechanism.
///
/// Methods that produce borrows are intentionally designed around [`Self::parts`]
/// (which does not borrow self beyond the call), so iterator objects can read
/// the pointer without conflicting with borrows of sibling fields.
pub enum ErasedQueryState {
    /// `QueryState<FilteredEntityRef>` - all components read-only.
    ReadOnly(*mut ()),
    /// `QueryState<FilteredEntityMut>` - at least one mutable component.
    Mutable(*mut ()),
}

impl ErasedQueryState {
    pub fn from_ref(qs: QueryState<FilteredEntityRef>) -> Self {
        Self::ReadOnly(Box::into_raw(Box::new(qs)) as *mut ())
    }

    pub fn from_mut(qs: QueryState<FilteredEntityMut>) -> Self {
        Self::Mutable(Box::into_raw(Box::new(qs)) as *mut ())
    }

    /// Returns (is_read_only, raw_pointer) for use in methods that need to
    /// avoid borrowing self (to prevent conflicts with other field borrows on
    /// the backend's iterator objects).
    pub fn parts(&self) -> (bool, *mut ()) {
        match self {
            Self::ReadOnly(p) => (true, *p),
            Self::Mutable(p) => (false, *p),
        }
    }

    /// Count matching entities (O(n) - iterates all).
    ///
    /// SAFETY comment discipline: declared access from `initialize` covers
    /// this state's access and the executor prevents conflicting systems from
    /// running concurrently, so the unchecked query has unique access to the
    /// components it reads.
    pub fn count(&self, cell: UnsafeWorldCell, last_run: Tick, this_run: Tick) -> usize {
        let (read_only, p) = self.parts();
        unsafe {
            if read_only {
                let qs = &mut *(p as *mut QueryState<FilteredEntityRef>);
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .iter_inner()
                    .count()
            } else {
                let qs = &mut *(p as *mut QueryState<FilteredEntityMut>);
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .iter_inner()
                    .count()
            }
        }
    }

    /// Check if no entities match.
    ///
    /// SAFETY: same discipline as [`Self::count`].
    pub fn is_empty_check(&self, cell: UnsafeWorldCell, last_run: Tick, this_run: Tick) -> bool {
        let (read_only, p) = self.parts();
        unsafe {
            if read_only {
                let qs = &mut *(p as *mut QueryState<FilteredEntityRef>);
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .is_empty()
            } else {
                let qs = &mut *(p as *mut QueryState<FilteredEntityMut>);
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .is_empty()
            }
        }
    }

    /// The Bevy-computed `FilteredAccess` for this state: the authoritative set
    /// of components the built query actually reads and writes. The debug access
    /// auditor compares it against the access `initialize` declared to the
    /// scheduler (derived independently from the ParamSpec via `QueryParamAccess`).
    ///
    /// SAFETY: reads only the QueryState's precomputed access metadata through
    /// the erased pointer (no world access); the pointer is valid for the life of
    /// the owning cache.
    #[cfg(debug_assertions)]
    pub fn component_access(&self) -> FilteredAccess {
        let (read_only, p) = self.parts();
        unsafe {
            if read_only {
                (*(p as *const QueryState<FilteredEntityRef>))
                    .component_access()
                    .clone()
            } else {
                (*(p as *const QueryState<FilteredEntityMut>))
                    .component_access()
                    .clone()
            }
        }
    }
}

// SAFETY: the raw QueryState pointer is only ever touched while the owning
// system runs on a single thread; the owning DynamicSystem types are already
// declared Send + Sync under the same discipline.
unsafe impl Send for ErasedQueryState {}
unsafe impl Sync for ErasedQueryState {}

impl Drop for ErasedQueryState {
    fn drop(&mut self) {
        let p = match self {
            Self::ReadOnly(p) | Self::Mutable(p) => *p,
        };
        if p.is_null() {
            return;
        }
        unsafe {
            match self {
                Self::ReadOnly(_) => {
                    let _ = Box::from_raw(p as *mut QueryState<FilteredEntityRef>);
                }
                Self::Mutable(_) => {
                    let _ = Box::from_raw(p as *mut QueryState<FilteredEntityMut>);
                }
            }
        }
    }
}

/// The backend-neutral half of a cached Query parameter: the erased
/// `QueryState` plus the tick-filter ids retained for the per-entity
/// Changed/Added check ([`passes_tick_filters`]). Built once in `initialize`
/// from the resolved [`QueryBuildSpec`].
pub struct CachedQueryCore {
    pub state: ErasedQueryState,
    /// ComponentIds for Changed\[T\] filters - entities must pass the
    /// per-entity tick check.
    pub changed_filter_ids: Vec<ComponentId>,
    /// ComponentIds for Added\[T\] filters - entities must pass the
    /// per-entity tick check.
    pub added_filter_ids: Vec<ComponentId>,
}

impl CachedQueryCore {
    /// Build with the state variant chosen by the spec: `FilteredEntityRef`
    /// for all-read-only queries, `FilteredEntityMut` otherwise (the pyo3
    /// backend's behavior).
    pub fn build_auto(world: &mut World, spec: &QueryBuildSpec) -> Self {
        let state = if spec.is_read_only() {
            ErasedQueryState::from_ref(build_query_state_ref(world, spec))
        } else {
            ErasedQueryState::from_mut(build_query_state(world, spec))
        };
        Self::with_state(state, spec)
    }

    /// Build with a `FilteredEntityMut` state unconditionally. The RustPython
    /// backend uses this because its per-entity extraction paths are not yet
    /// variant-aware; migrating it to [`Self::build_auto`] is a known
    /// follow-up of the extraction campaign.
    pub fn build_mut(world: &mut World, spec: &QueryBuildSpec) -> Self {
        Self::with_state(
            ErasedQueryState::from_mut(build_query_state(world, spec)),
            spec,
        )
    }

    fn with_state(state: ErasedQueryState, spec: &QueryBuildSpec) -> Self {
        Self {
            state,
            changed_filter_ids: spec.changed_filters.clone(),
            added_filter_ids: spec.added_filters.clone(),
        }
    }

    /// True if the query carries Added/Changed filters, i.e. iteration must
    /// run the per-entity tick check.
    pub fn has_tick_filters(&self) -> bool {
        !self.changed_filter_ids.is_empty() || !self.added_filter_ids.is_empty()
    }

    /// Check whether an entity passes this query's Added/Changed tick filters.
    pub fn entity_passes_tick_filters(
        &self,
        get_ticks: impl Fn(ComponentId) -> Option<ComponentTicks>,
        last_run: Tick,
        this_run: Tick,
    ) -> bool {
        passes_tick_filters(
            get_ticks,
            &self.changed_filter_ids,
            &self.added_filter_ids,
            last_run,
            this_run,
        )
    }
}

/// Check whether an entity passes Added/Changed tick filters.
///
/// Generic over the entity type via a closure that provides
/// `get_change_ticks_by_id`. Used both through entity-access wrappers (for
/// next/get/single/iter_many) and through raw `FilteredEntityRef`/
/// `FilteredEntityMut` items (for len/is_empty via manual iteration).
#[inline]
pub fn passes_tick_filters(
    get_ticks: impl Fn(ComponentId) -> Option<ComponentTicks>,
    changed_ids: &[ComponentId],
    added_ids: &[ComponentId],
    last_run: Tick,
    this_run: Tick,
) -> bool {
    if changed_ids.is_empty() && added_ids.is_empty() {
        return true;
    }

    for &id in changed_ids {
        if let Some(ticks) = get_ticks(id) {
            if !ticks.is_changed(last_run, this_run) {
                return false;
            }
        }
    }

    for &id in added_ids {
        if let Some(ticks) = get_ticks(id) {
            if !ticks.is_added(last_run, this_run) {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use bevy::ecs::component::Component;

    use super::{super::query_builder_ext::QueryComponent, *};

    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;

    fn spec(world: &mut World, mutable: bool, changed: bool) -> QueryBuildSpec {
        let a = world.register_component::<A>();
        let b = world.register_component::<B>();
        QueryBuildSpec {
            components: vec![QueryComponent {
                id: a,
                optional: false,
                mutable,
            }],
            with_filters: Vec::new(),
            without_filters: Vec::new(),
            changed_filters: if changed { vec![b] } else { Vec::new() },
            added_filters: Vec::new(),
            anyof_filters: Vec::new(),
        }
    }

    #[test]
    fn build_auto_picks_ref_state_for_read_only() {
        let mut world = World::new();
        let spec = spec(&mut world, false, false);
        let core = CachedQueryCore::build_auto(&mut world, &spec);
        let (read_only, _) = core.state.parts();
        assert!(read_only);
    }

    #[test]
    fn build_auto_picks_mut_state_for_writes() {
        let mut world = World::new();
        let spec = spec(&mut world, true, false);
        let core = CachedQueryCore::build_auto(&mut world, &spec);
        let (read_only, _) = core.state.parts();
        assert!(!read_only);
    }

    #[test]
    fn build_mut_is_always_mutable() {
        let mut world = World::new();
        let spec = spec(&mut world, false, false);
        let core = CachedQueryCore::build_mut(&mut world, &spec);
        let (read_only, _) = core.state.parts();
        assert!(!read_only);
    }

    #[test]
    fn tick_filter_ids_retained_from_spec() {
        let mut world = World::new();
        let spec = spec(&mut world, false, true);
        let core = CachedQueryCore::build_auto(&mut world, &spec);
        assert!(core.has_tick_filters());
        assert_eq!(core.changed_filter_ids, spec.changed_filters);

        let no_filters = CachedQueryCore::build_auto(
            &mut world,
            &QueryBuildSpec {
                components: spec.components.clone(),
                with_filters: Vec::new(),
                without_filters: Vec::new(),
                changed_filters: Vec::new(),
                added_filters: Vec::new(),
                anyof_filters: Vec::new(),
            },
        );
        assert!(!no_filters.has_tick_filters());
    }

    #[test]
    fn count_and_is_empty_track_matching_entities() {
        let mut world = World::new();
        let spec = spec(&mut world, false, false);
        let core = CachedQueryCore::build_auto(&mut world, &spec);

        let tick = world.change_tick();
        let cell = world.as_unsafe_world_cell_readonly();
        assert!(core.state.is_empty_check(cell, Tick::new(0), tick));
        assert_eq!(core.state.count(cell, Tick::new(0), tick), 0);

        world.spawn(A);
        world.spawn((A, B));
        world.spawn(B);
        let tick = world.change_tick();
        let cell = world.as_unsafe_world_cell_readonly();
        assert!(!core.state.is_empty_check(cell, Tick::new(0), tick));
        assert_eq!(core.state.count(cell, Tick::new(0), tick), 2);
    }

    #[test]
    fn passes_tick_filters_semantics() {
        let mut world = World::new();
        let a = world.register_component::<A>();
        let entity = world.spawn(A).id();
        let spawn_tick = world.change_tick();

        // Advance the world a few ticks so the spawn is no longer "current".
        for _ in 0..3 {
            world.increment_change_tick();
        }
        let this_run = world.change_tick();

        let entity_ref = world.entity(entity);
        let get_ticks = |id: ComponentId| entity_ref.get_change_ticks_by_id(id);

        // last_run before the spawn: A counts as added and changed.
        assert!(passes_tick_filters(
            get_ticks,
            &[a],
            &[],
            Tick::new(0),
            this_run
        ));
        assert!(passes_tick_filters(
            get_ticks,
            &[],
            &[a],
            Tick::new(0),
            this_run
        ));

        // last_run after the spawn: neither changed nor added since.
        assert!(!passes_tick_filters(
            get_ticks,
            &[a],
            &[],
            spawn_tick,
            this_run
        ));
        assert!(!passes_tick_filters(
            get_ticks,
            &[],
            &[a],
            spawn_tick,
            this_run
        ));

        // No filters: always passes.
        assert!(passes_tick_filters(
            get_ticks,
            &[],
            &[],
            spawn_tick,
            this_run
        ));
    }
}
