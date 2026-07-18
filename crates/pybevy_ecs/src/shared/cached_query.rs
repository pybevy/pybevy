//! Shared per-Query cached state: the type-erased `QueryState` plus the
//! per-entity Changed/Added tick check.
//!
//! Both backends build one [`CachedQueryCore`] per Query parameter in
//! `System::initialize` and reuse it across every run. The backend keeps its
//! own extraction plan (how entities become Python objects) next to the core;
//! everything below is interpreter-free.
//!
//! Per-run iterator erasure and lookup orchestration live in the sibling
//! `query_runtime` module; each backend keeps only its row materializer.

#[cfg(debug_assertions)]
use bevy::ecs::query::FilteredAccess;
use bevy::ecs::{
    change_detection::{ComponentTicks, Tick},
    component::ComponentId,
    query::QueryState,
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

    /// Reconstitute the concrete `QueryState` behind the erased pointer and run
    /// the branch selected by the variant tag. This is the single place that
    /// performs the `*mut () as *mut QueryState<_>` cast, so a mistagged cast
    /// (which would be UB) cannot be duplicated across call sites.
    ///
    /// # Safety
    ///
    /// The erased pointer must still own a live `QueryState` of the tagged
    /// variant, and the caller must uphold whatever contract the closures rely
    /// on (typically the world/access/aliasing requirements of
    /// `query_unchecked_with_ticks`).
    /// The `'s` lifetime is chosen by the caller: because the state borrow is
    /// reconstituted from a raw pointer, a caller returning a value that borrows
    /// the state (e.g. `get_inner`) can tie `'s` to the world lifetime, while a
    /// caller returning an owned value leaves it minimal.
    pub(crate) unsafe fn with_state<'s, R>(
        &self,
        read_only: impl FnOnce(&'s mut QueryState<FilteredEntityRef<'static, 'static>>) -> R,
        mutable: impl FnOnce(&'s mut QueryState<FilteredEntityMut<'static, 'static>>) -> R,
    ) -> R {
        let (is_read_only, p) = self.parts();
        if is_read_only {
            // SAFETY: `parts` tags this pointer as the read-only variant; the
            // caller upholds liveness for `'s` and the closure's own preconditions.
            read_only(unsafe { &mut *(p as *mut QueryState<FilteredEntityRef<'static, 'static>>) })
        } else {
            // SAFETY: `parts` tags this pointer as the mutable variant; the
            // caller upholds liveness for `'s` and the closure's own preconditions.
            mutable(unsafe { &mut *(p as *mut QueryState<FilteredEntityMut<'static, 'static>>) })
        }
    }

    /// Count matching entities by draining a fresh unchecked traversal (O(n)).
    ///
    /// # Safety
    ///
    /// `cell` must reference the world this state was built from, and the
    /// scheduler's declared access must cover this state so the unchecked query
    /// has unique access to the components it touches. No other live iterator or
    /// query may borrow this same `QueryState` for the duration of the call.
    pub unsafe fn count(&self, cell: UnsafeWorldCell, last_run: Tick, this_run: Tick) -> usize {
        // SAFETY: the caller guarantees `cell` matches this state's world, declared
        // access covers it, and no other iterator/query aliases it; the closures'
        // `query_unchecked_with_ticks` calls inherit that contract.
        unsafe {
            self.with_state(
                |qs| {
                    qs.query_unchecked_with_ticks(cell, last_run, this_run)
                        .iter_inner()
                        .count()
                },
                |qs| {
                    qs.query_unchecked_with_ticks(cell, last_run, this_run)
                        .iter_inner()
                        .count()
                },
            )
        }
    }

    /// True when no entities match, via a fresh unchecked traversal.
    ///
    /// # Safety
    ///
    /// Same world/access/aliasing contract as [`Self::count`].
    pub unsafe fn is_empty_check(
        &self,
        cell: UnsafeWorldCell,
        last_run: Tick,
        this_run: Tick,
    ) -> bool {
        // SAFETY: same contract as `count`; the closures' `query_unchecked_with_ticks`
        // calls inherit the caller's world/access/aliasing guarantee.
        unsafe {
            self.with_state(
                |qs| {
                    qs.query_unchecked_with_ticks(cell, last_run, this_run)
                        .is_empty()
                },
                |qs| {
                    qs.query_unchecked_with_ticks(cell, last_run, this_run)
                        .is_empty()
                },
            )
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
        // SAFETY: this reads precomputed metadata through a pointer owned by
        // the live cache and does not access the world.
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

impl Drop for ErasedQueryState {
    fn drop(&mut self) {
        let p = match self {
            Self::ReadOnly(p) | Self::Mutable(p) => *p,
        };
        if p.is_null() {
            return;
        }
        // SAFETY: this state uniquely owns the allocation and the enum variant
        // records the concrete type used to create it.
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
    pub(crate) state: ErasedQueryState,
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

    /// Build with a `FilteredEntityMut` state unconditionally. This preserves
    /// compatibility with adapter extraction paths that are not yet
    /// variant-aware; migrating them to [`Self::build_auto`] is a known
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

    /// Return the Bevy-computed component access for debug access auditing.
    #[cfg(debug_assertions)]
    pub fn component_access(&self) -> FilteredAccess {
        self.state.component_access()
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
        if let Some(ticks) = get_ticks(id)
            && !ticks.is_changed(last_run, this_run)
        {
            return false;
        }
    }

    for &id in added_ids {
        if let Some(ticks) = get_ticks(id)
            && !ticks.is_added(last_run, this_run)
        {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{change_detection::ComponentTicks, component::Component};

    use super::{super::query_builder_ext::QueryComponent, *};

    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;

    /// Helper: build ComponentTicks with explicit added/changed ticks.
    fn ticks(added: u32, changed: u32) -> ComponentTicks {
        ComponentTicks {
            added: Tick::new(added),
            changed: Tick::new(changed),
        }
    }

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
        // SAFETY: `cell` is the world `core` was built from, no other query borrows it.
        unsafe {
            assert!(core.state.is_empty_check(cell, Tick::new(0), tick));
            assert_eq!(core.state.count(cell, Tick::new(0), tick), 0);
        }

        world.spawn(A);
        world.spawn((A, B));
        world.spawn(B);
        let tick = world.change_tick();
        let cell = world.as_unsafe_world_cell_readonly();
        // SAFETY: `cell` is the world `core` was built from, no other query borrows it.
        unsafe {
            assert!(!core.state.is_empty_check(cell, Tick::new(0), tick));
            assert_eq!(core.state.count(cell, Tick::new(0), tick), 2);
        }
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

    #[test]
    fn no_filters_always_passes() {
        assert!(passes_tick_filters(
            |_| None,
            &[],
            &[],
            Tick::new(0),
            Tick::new(1)
        ));
    }

    #[test]
    fn changed_filter_passes_when_changed_after_last_run() {
        let id = ComponentId::new(0);
        // changed tick 5 > last_run 3 - should pass
        assert!(passes_tick_filters(
            |_| Some(ticks(1, 5)),
            &[id],
            &[],
            Tick::new(3),
            Tick::new(6),
        ));
    }

    #[test]
    fn changed_filter_fails_when_not_changed_since_last_run() {
        let id = ComponentId::new(0);
        // changed tick 2 <= last_run 3 - should fail
        assert!(!passes_tick_filters(
            |_| Some(ticks(1, 2)),
            &[id],
            &[],
            Tick::new(3),
            Tick::new(6),
        ));
    }

    #[test]
    fn added_filter_passes_when_added_after_last_run() {
        let id = ComponentId::new(0);
        // added tick 5 > last_run 3 - should pass
        assert!(passes_tick_filters(
            |_| Some(ticks(5, 5)),
            &[],
            &[id],
            Tick::new(3),
            Tick::new(6),
        ));
    }

    #[test]
    fn added_filter_fails_when_not_added_since_last_run() {
        let id = ComponentId::new(0);
        // added tick 1 <= last_run 3 - should fail
        assert!(!passes_tick_filters(
            |_| Some(ticks(1, 5)),
            &[],
            &[id],
            Tick::new(3),
            Tick::new(6),
        ));
    }

    #[test]
    fn multiple_changed_all_pass() {
        let ids = [ComponentId::new(0), ComponentId::new(1)];
        // Both changed after last_run
        let data = [ticks(1, 5), ticks(1, 4)];
        assert!(passes_tick_filters(
            |id| Some(data[id.index()]),
            &ids,
            &[],
            Tick::new(3),
            Tick::new(6),
        ));
    }

    #[test]
    fn multiple_changed_one_stale_fails() {
        let ids = [ComponentId::new(0), ComponentId::new(1)];
        // id 0 changed at 5 (passes), id 1 changed at 2 (stale - fails)
        let data = [ticks(1, 5), ticks(1, 2)];
        assert!(!passes_tick_filters(
            |id| Some(data[id.index()]),
            &ids,
            &[],
            Tick::new(3),
            Tick::new(6),
        ));
    }

    #[test]
    fn missing_ticks_passes_filter() {
        // Component not present in entity (None) - filter is skipped for that component
        let id = ComponentId::new(0);
        assert!(passes_tick_filters(
            |_| None,
            &[id],
            &[],
            Tick::new(3),
            Tick::new(6),
        ));
    }

    #[test]
    fn both_added_and_changed_must_pass() {
        let changed_id = ComponentId::new(0);
        let added_id = ComponentId::new(1);
        // changed: tick 5 > last_run 3 - passes
        // added: tick 5 > last_run 3 - passes
        let data = [ticks(1, 5), ticks(5, 5)];
        assert!(passes_tick_filters(
            |id| Some(data[id.index()]),
            &[changed_id],
            &[added_id],
            Tick::new(3),
            Tick::new(6),
        ));
    }

    #[test]
    fn changed_passes_but_added_fails() {
        let changed_id = ComponentId::new(0);
        let added_id = ComponentId::new(1);
        // changed: tick 5 > last_run 3 - passes
        // added: tick 1 <= last_run 3 - fails
        let data = [ticks(1, 5), ticks(1, 5)];
        assert!(!passes_tick_filters(
            |id| Some(data[id.index()]),
            &[changed_id],
            &[added_id],
            Tick::new(3),
            Tick::new(6),
        ));
    }
}
