//! Interpreter-neutral query iteration and lookup orchestration.
//!
//! [`QueryRuntimeCore`] owns the per-run world cell, tick window, lazy erased
//! iterator, and validity fence. Backends retain only their row materializers:
//! turning a [`FilteredEntityAccess`] into interpreter objects and mapping
//! [`QueryRuntimeError`] into the appropriate Python exception.

use std::{cell::Cell, fmt, ptr::NonNull};

use bevy::ecs::{
    change_detection::Tick,
    entity::Entity,
    query::{QueryIter, QueryState},
    world::{FilteredEntityMut, FilteredEntityRef, World, unsafe_world_cell::UnsafeWorldCell},
};
use pybevy_storage::{FilteredEntityAccess, StorageError, ValidityFlag};

use super::cached_query::{CachedQueryCore, ErasedQueryState};

const NESTED_ITERATION_MESSAGE: &str = "Cannot nest iteration on the same Query (Bevy disallows this via borrow rules). \
     Collect into a list first: items = list(query)";
const REENTRANT_OPERATION_MESSAGE: &str =
    "Cannot re-enter a Query operation while entity access is active";
const NO_ENTITIES_MESSAGE: &str = "Query returned no entities. Expected exactly one.";
const MULTIPLE_ENTITIES_MESSAGE: &str = "Query returned multiple entities. Expected exactly one.";

type ReadOnlyIter = QueryIter<'static, 'static, FilteredEntityRef<'static, 'static>, ()>;
type MutableIter = QueryIter<'static, 'static, FilteredEntityMut<'static, 'static>, ()>;

/// Backend-neutral failures produced while orchestrating a query operation.
#[derive(Debug, Clone)]
pub enum QueryRuntimeError {
    /// The query escaped its system execution window.
    Storage(StorageError),
    /// A second iteration began before the current one was exhausted.
    NestedIteration,
    /// A query operation was re-entered while another operation held entity access.
    ReentrantOperation,
    /// A single-result operation matched no entities.
    NoEntities,
    /// A single-result operation matched more than one entity.
    MultipleEntities,
}

impl fmt::Display for QueryRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => error.fmt(formatter),
            Self::NestedIteration => formatter.write_str(NESTED_ITERATION_MESSAGE),
            Self::ReentrantOperation => formatter.write_str(REENTRANT_OPERATION_MESSAGE),
            Self::NoEntities => formatter.write_str(NO_ENTITIES_MESSAGE),
            Self::MultipleEntities => formatter.write_str(MULTIPLE_ENTITIES_MESSAGE),
        }
    }
}

impl std::error::Error for QueryRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

impl From<StorageError> for QueryRuntimeError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

/// Backend leaf that turns one matched entity into an interpreter row object.
///
/// `Context` carries the interpreter handle (`Python<'py>` or
/// `&VirtualMachine`), keeping it out of the neutral runtime.
pub trait RowMaterializer<Context> {
    /// Interpreter-specific row object.
    type Output;
    /// Interpreter-specific exception type.
    type Error;

    /// Materialize one matched entity.
    fn materialize(
        &self,
        entity: &mut FilteredEntityAccess<'_, '_>,
        context: Context,
    ) -> Result<Self::Output, Self::Error>;
}

/// Failure from a combined neutral-runtime and backend-materialization call.
#[derive(Debug)]
pub enum QueryExecutionError<BackendError> {
    /// Query traversal, validity, or cardinality failure.
    Runtime(QueryRuntimeError),
    /// Backend row construction failure.
    Materialize(BackendError),
}

impl<BackendError: fmt::Display> fmt::Display for QueryExecutionError<BackendError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => error.fmt(formatter),
            Self::Materialize(error) => error.fmt(formatter),
        }
    }
}

impl<BackendError> From<QueryRuntimeError> for QueryExecutionError<BackendError> {
    fn from(error: QueryRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

/// A lifetime-erased iterator pointer tagged with its concrete query variant.
#[derive(Clone, Copy)]
enum ErasedQueryIter {
    ReadOnly(*mut ()),
    Mutable(*mut ()),
}

impl ErasedQueryIter {
    /// Advance the erased iterator.
    ///
    /// # Safety
    ///
    /// The iterator must still be live, its world cell must be valid, and the
    /// returned access must not outlive the system execution window.
    unsafe fn next<'a>(self) -> Option<FilteredEntityAccess<'a, 'a>> {
        match self {
            Self::ReadOnly(pointer) => {
                // SAFETY: guaranteed by the caller and the variant tag.
                let iterator = unsafe { &mut *(pointer as *mut ReadOnlyIter) };
                iterator.next().map(FilteredEntityAccess::Ref)
            }
            Self::Mutable(pointer) => {
                // SAFETY: guaranteed by the caller and the variant tag.
                let iterator = unsafe { &mut *(pointer as *mut MutableIter) };
                iterator.next().map(FilteredEntityAccess::Mut)
            }
        }
    }

    /// Reconstruct and drop the allocation represented by this pointer.
    ///
    /// # Safety
    ///
    /// The pointer must have come from [`create_iter`] and must be dropped once.
    unsafe fn drop_allocation(self) {
        match self {
            Self::ReadOnly(pointer) => {
                // SAFETY: guaranteed by the caller and the variant tag.
                let _ = unsafe { Box::from_raw(pointer as *mut ReadOnlyIter) };
            }
            Self::Mutable(pointer) => {
                // SAFETY: guaranteed by the caller and the variant tag.
                let _ = unsafe { Box::from_raw(pointer as *mut MutableIter) };
            }
        }
    }
}

/// Create a fresh iterator for an erased cached query state.
///
/// This uses `query_unchecked_with_ticks`, which refreshes the cached state's
/// archetypes internally, so entities spawned after system initialization are
/// visible without rebuilding the query state.
///
/// # Safety
///
/// `state` must have been built from `cell`'s world. The scheduler's declared
/// access must cover the query, and the caller must fence the returned iterator
/// with the system's validity window.
unsafe fn create_iter(
    state: &ErasedQueryState,
    cell: UnsafeWorldCell<'static>,
    last_run: Tick,
    this_run: Tick,
) -> ErasedQueryIter {
    let (read_only, pointer) = state.parts();
    if read_only {
        // SAFETY: `parts` tags this pointer as the read-only state variant.
        let state = unsafe { &mut *(pointer as *mut QueryState<FilteredEntityRef>) };
        // SAFETY: the caller guarantees the world and scheduler-access contract.
        let iterator =
            unsafe { state.query_unchecked_with_ticks(cell, last_run, this_run) }.iter_inner();
        ErasedQueryIter::ReadOnly(Box::into_raw(Box::new(iterator)) as *mut ())
    } else {
        // SAFETY: `parts` tags this pointer as the mutable state variant.
        let state = unsafe { &mut *(pointer as *mut QueryState<FilteredEntityMut>) };
        // SAFETY: the caller guarantees the world and scheduler-access contract.
        let iterator =
            unsafe { state.query_unchecked_with_ticks(cell, last_run, this_run) }.iter_inner();
        ErasedQueryIter::Mutable(Box::into_raw(Box::new(iterator)) as *mut ())
    }
}

/// Look up one entity through an erased cached query state.
///
/// # Safety
///
/// The state/world/access requirements are the same as [`create_iter`].
unsafe fn get_entity<'a>(
    state: &ErasedQueryState,
    cell: UnsafeWorldCell<'a>,
    last_run: Tick,
    this_run: Tick,
    entity: Entity,
) -> Option<FilteredEntityAccess<'a, 'a>> {
    let (read_only, pointer) = state.parts();
    if read_only {
        // SAFETY: `parts` tags this pointer as the read-only state variant.
        let state = unsafe { &mut *(pointer as *mut QueryState<FilteredEntityRef>) };
        // SAFETY: the caller guarantees the world and scheduler-access contract.
        unsafe { state.query_unchecked_with_ticks(cell, last_run, this_run) }
            .get_inner(entity)
            .ok()
            .map(FilteredEntityAccess::Ref)
    } else {
        // SAFETY: `parts` tags this pointer as the mutable state variant.
        let state = unsafe { &mut *(pointer as *mut QueryState<FilteredEntityMut>) };
        // SAFETY: the caller guarantees the world and scheduler-access contract.
        unsafe { state.query_unchecked_with_ticks(cell, last_run, this_run) }
            .get_inner(entity)
            .ok()
            .map(FilteredEntityAccess::Mut)
    }
}

/// Per-run, interpreter-neutral query runtime.
///
/// The owning backend stores its extraction plan next to this core. A regular
/// system points at the `CachedQueryCore` owned by its `DynamicSystem`; observer
/// paths may point into a boxed backend cache. `None` represents a query whose
/// required component types could not be resolved and therefore never matches.
pub struct QueryRuntimeCore {
    cached: Option<NonNull<CachedQueryCore>>,
    world_cell: UnsafeWorldCell<'static>,
    iterator: Cell<Option<ErasedQueryIter>>,
    validity: ValidityFlag,
    iterating: Cell<bool>,
    operation_active: Cell<bool>,
    last_run: Tick,
    this_run: Tick,
}

impl QueryRuntimeCore {
    /// Construct a runtime over a cached query and a system-run world cell.
    ///
    /// # Safety
    ///
    /// A non-empty `cached` value must remain at a stable address until this
    /// runtime is dropped. `world_cell` must reference the world from which the
    /// cache was built. Both must remain live while `validity` permits access.
    /// No other runtime may operate on the same cached state concurrently.
    pub unsafe fn new(
        cached: Option<&CachedQueryCore>,
        world_cell: UnsafeWorldCell,
        validity: ValidityFlag,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        // SAFETY: the caller fences this lifetime-erased cell with `validity`.
        let world_cell = unsafe {
            std::mem::transmute::<UnsafeWorldCell<'_>, UnsafeWorldCell<'static>>(world_cell)
        };
        Self {
            cached: cached.map(NonNull::from),
            world_cell,
            iterator: Cell::new(None),
            validity,
            iterating: Cell::new(false),
            operation_active: Cell::new(false),
            last_run,
            this_run,
        }
    }

    /// Return the shared validity flag used by row materializers.
    pub fn validity(&self) -> &ValidityFlag {
        &self.validity
    }

    /// Check that this runtime is still inside its system execution window.
    pub fn check_valid(&self) -> Result<(), QueryRuntimeError> {
        self.validity.check().map_err(Into::into)
    }

    /// Return a momentary raw world pointer for proxy write-back paths.
    ///
    /// # Safety
    ///
    /// This compatibility escape hatch derives a whole-world pointer while row
    /// access may be live. Callers must keep dereferences inside the validity
    /// window and declared scheduler access. The remaining aliasing limitation
    /// of this path is documented in `docs/safety.md`.
    pub unsafe fn world_ptr(&self) -> Result<*mut World, QueryRuntimeError> {
        self.validity.check()?;
        // SAFETY: this preserves the documented compatibility path; validity
        // and scheduler access constrain use but do not narrow the whole world.
        Ok(unsafe { self.world_cell.world_mut() as *mut World })
    }

    /// Start a fresh Python-level iteration.
    pub fn begin_iteration(&self) -> Result<(), QueryRuntimeError> {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        if self.iterating.get() {
            return Err(QueryRuntimeError::NestedIteration);
        }
        self.clear_iterator();
        Ok(())
    }

    /// Advance the lazy iterator and return the next entity passing tick filters.
    ///
    /// # Safety
    ///
    /// The caller must hold this runtime's operation guard until the returned
    /// access is dropped, preventing overlapping mutable entity access.
    unsafe fn next_entity(
        &self,
    ) -> Result<Option<FilteredEntityAccess<'_, '_>>, QueryRuntimeError> {
        let Some(cached) = self.cached() else {
            self.iterating.set(false);
            return Ok(None);
        };

        self.iterating.set(true);
        if self.iterator.get().is_none() {
            // SAFETY: the constructor's contract binds this cache to this cell,
            // validity is checked above, and scheduler access covers the query.
            let iterator = unsafe {
                create_iter(&cached.state, self.world_cell, self.last_run, self.this_run)
            };
            self.iterator.set(Some(iterator));
        }

        let iterator = self.iterator.get().expect("iterator initialized above");
        loop {
            // SAFETY: the allocation is owned by `self.iterator`, validity was
            // checked above, and the returned borrow remains within this run.
            match unsafe { iterator.next() } {
                Some(access) if self.passes_tick_filters(cached, &access) => {
                    return Ok(Some(access));
                }
                Some(_) => {}
                None => {
                    self.iterating.set(false);
                    return Ok(None);
                }
            }
        }
    }

    /// Advance and materialize one row through the backend leaf.
    pub fn next_with<Context, Materializer>(
        &self,
        materializer: &Materializer,
        context: Context,
    ) -> Result<Option<Materializer::Output>, QueryExecutionError<Materializer::Error>>
    where
        Materializer: RowMaterializer<Context>,
    {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        // SAFETY: `_operation` prevents another operation from overlapping the
        // entity access, which is consumed by the materializer before it drops.
        let Some(mut entity) = (unsafe { self.next_entity()? }) else {
            return Ok(None);
        };
        materializer
            .materialize(&mut entity, context)
            .map(Some)
            .map_err(QueryExecutionError::Materialize)
    }

    /// Count matching entities, including Added/Changed filtering.
    pub fn count(&self) -> Result<usize, QueryRuntimeError> {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        let Some(cached) = self.cached() else {
            return Ok(0);
        };
        if !cached.has_tick_filters() {
            return Ok(cached
                .state
                .count(self.world_cell, self.last_run, self.this_run));
        }

        let mut count = 0;
        // SAFETY: the constructor contract and validity check establish the
        // world/cache/access requirements for this temporary iterator.
        let iterator =
            unsafe { create_iter(&cached.state, self.world_cell, self.last_run, self.this_run) };
        // The guard ensures the erased allocation is reclaimed on every return.
        let iterator = ErasedIterGuard(iterator);
        loop {
            // SAFETY: the guard owns the live iterator for this method scope.
            match unsafe { iterator.0.next() } {
                Some(access) if self.passes_tick_filters(cached, &access) => count += 1,
                Some(_) => {}
                None => return Ok(count),
            }
        }
    }

    /// Return whether no entities match, including Added/Changed filtering.
    pub fn is_empty(&self) -> Result<bool, QueryRuntimeError> {
        self.validity.check()?;
        let _operation = self.enter_operation()?;
        let Some(cached) = self.cached() else {
            return Ok(true);
        };
        if !cached.has_tick_filters() {
            return Ok(cached
                .state
                .is_empty_check(self.world_cell, self.last_run, self.this_run));
        }

        // SAFETY: the constructor contract and validity check establish the
        // world/cache/access requirements for this temporary iterator.
        let iterator =
            unsafe { create_iter(&cached.state, self.world_cell, self.last_run, self.this_run) };
        let iterator = ErasedIterGuard(iterator);
        loop {
            // SAFETY: the guard owns the live iterator for this method scope.
            match unsafe { iterator.0.next() } {
                Some(access) if self.passes_tick_filters(cached, &access) => return Ok(false),
                Some(_) => {}
                None => return Ok(true),
            }
        }
    }

    /// Look up an entity and return it only when all query filters pass.
    ///
    /// # Safety
    ///
    /// The caller must hold this runtime's operation guard until the returned
    /// access is dropped, preventing overlapping mutable entity access.
    unsafe fn get_entity_access(
        &self,
        entity: Entity,
    ) -> Result<Option<FilteredEntityAccess<'_, '_>>, QueryRuntimeError> {
        let Some(cached) = self.cached() else {
            return Ok(None);
        };
        // SAFETY: the constructor contract and validity check establish the
        // world/cache/access requirements for this lookup.
        let result = unsafe {
            get_entity(
                &cached.state,
                self.world_cell,
                self.last_run,
                self.this_run,
                entity,
            )
        };
        Ok(result.filter(|access| self.passes_tick_filters(cached, access)))
    }

    /// Look up and materialize one entity through the backend leaf.
    pub fn get_with<Context, Materializer>(
        &self,
        entity: Entity,
        materializer: &Materializer,
        context: Context,
    ) -> Result<Option<Materializer::Output>, QueryExecutionError<Materializer::Error>>
    where
        Materializer: RowMaterializer<Context>,
    {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        // SAFETY: `_operation` prevents another operation from overlapping the
        // entity access, which is consumed by the materializer before it drops.
        let Some(mut entity) = (unsafe { self.get_entity_access(entity)? }) else {
            return Ok(None);
        };
        materializer
            .materialize(&mut entity, context)
            .map(Some)
            .map_err(QueryExecutionError::Materialize)
    }

    /// Return exactly one matching entity or a neutral cardinality error.
    ///
    /// # Safety
    ///
    /// The caller must hold this runtime's operation guard until the returned
    /// access is dropped, preventing overlapping mutable entity access.
    unsafe fn single_entity(&self) -> Result<FilteredEntityAccess<'_, '_>, QueryRuntimeError> {
        let Some(cached) = self.cached() else {
            return Err(QueryRuntimeError::NoEntities);
        };

        // SAFETY: the constructor contract and validity check establish the
        // world/cache/access requirements for this temporary iterator.
        let iterator =
            unsafe { create_iter(&cached.state, self.world_cell, self.last_run, self.this_run) };
        let iterator = ErasedIterGuard(iterator);
        let first = loop {
            // SAFETY: the guard owns the live iterator for this method scope.
            match unsafe { iterator.0.next() } {
                Some(access) if self.passes_tick_filters(cached, &access) => break access,
                Some(_) => {}
                None => return Err(QueryRuntimeError::NoEntities),
            }
        };
        loop {
            // SAFETY: the guard owns the live iterator for this method scope.
            match unsafe { iterator.0.next() } {
                Some(access) if self.passes_tick_filters(cached, &access) => {
                    return Err(QueryRuntimeError::MultipleEntities);
                }
                Some(_) => {}
                None => return Ok(first),
            }
        }
    }

    /// Enforce single-result cardinality and materialize the row.
    pub fn single_with<Context, Materializer>(
        &self,
        materializer: &Materializer,
        context: Context,
    ) -> Result<Materializer::Output, QueryExecutionError<Materializer::Error>>
    where
        Materializer: RowMaterializer<Context>,
    {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        // SAFETY: `_operation` prevents another operation from overlapping the
        // entity access, which is consumed by the materializer before it drops.
        let mut entity = unsafe { self.single_entity()? };
        materializer
            .materialize(&mut entity, context)
            .map_err(QueryExecutionError::Materialize)
    }

    fn cached(&self) -> Option<&CachedQueryCore> {
        self.cached.map(|pointer| {
            // SAFETY: guaranteed by the constructor; validity is checked before
            // every public operation that calls this helper.
            unsafe { pointer.as_ref() }
        })
    }

    fn passes_tick_filters(&self, cached: &CachedQueryCore, access: &FilteredEntityAccess) -> bool {
        cached.entity_passes_tick_filters(
            |component_id| access.get_change_ticks_by_id(component_id),
            self.last_run,
            self.this_run,
        )
    }

    fn enter_operation(&self) -> Result<OperationGuard<'_>, QueryRuntimeError> {
        if self.operation_active.replace(true) {
            return Err(QueryRuntimeError::ReentrantOperation);
        }
        Ok(OperationGuard {
            active: &self.operation_active,
        })
    }

    fn clear_iterator(&self) {
        if let Some(iterator) = self.iterator.take() {
            // SAFETY: `self.iterator` uniquely owns this allocation.
            unsafe { iterator.drop_allocation() };
        }
    }
}

// SAFETY: the raw cache/world pointers and interior-mutability cells are only
// accessed while the owning system runs on one executor thread. The scheduler's
// access set prevents conflicts, and `ValidityFlag` fences the run lifetime.
unsafe impl Send for QueryRuntimeCore {}

impl Drop for QueryRuntimeCore {
    fn drop(&mut self) {
        self.clear_iterator();
    }
}

/// RAII owner for a temporary erased iterator.
struct ErasedIterGuard(ErasedQueryIter);

impl Drop for ErasedIterGuard {
    fn drop(&mut self) {
        // SAFETY: this guard uniquely owns the allocation.
        unsafe { self.0.drop_allocation() };
    }
}

/// Resets the operation fence on normal returns, errors, and unwinding.
struct OperationGuard<'a> {
    active: &'a Cell<bool>,
}

impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        self.active.set(false);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::component::Component;

    use super::{super::query_builder_ext::QueryComponent, *};
    use crate::shared::query_builder_ext::QueryBuildSpec;

    #[derive(Component)]
    struct A(u32);

    struct EntityMaterializer;

    impl RowMaterializer<()> for EntityMaterializer {
        type Output = Entity;
        type Error = std::convert::Infallible;

        fn materialize(
            &self,
            entity: &mut FilteredEntityAccess<'_, '_>,
            _context: (),
        ) -> Result<Self::Output, Self::Error> {
            Ok(entity.id())
        }
    }

    struct IsMutableMaterializer;

    impl RowMaterializer<()> for IsMutableMaterializer {
        type Output = bool;
        type Error = std::convert::Infallible;

        fn materialize(
            &self,
            entity: &mut FilteredEntityAccess<'_, '_>,
            _context: (),
        ) -> Result<Self::Output, Self::Error> {
            Ok(matches!(entity, FilteredEntityAccess::Mut(_)))
        }
    }

    struct ReentrantMaterializer<'a> {
        runtime: &'a QueryRuntimeCore,
    }

    impl RowMaterializer<()> for ReentrantMaterializer<'_> {
        type Output = Entity;
        type Error = std::convert::Infallible;

        fn materialize(
            &self,
            entity: &mut FilteredEntityAccess<'_, '_>,
            _context: (),
        ) -> Result<Self::Output, Self::Error> {
            assert!(matches!(
                self.runtime.count(),
                Err(QueryRuntimeError::ReentrantOperation)
            ));
            Ok(entity.id())
        }
    }

    struct FailingMaterializer;

    impl RowMaterializer<()> for FailingMaterializer {
        type Output = Entity;
        type Error = ();

        fn materialize(
            &self,
            _entity: &mut FilteredEntityAccess<'_, '_>,
            _context: (),
        ) -> Result<Self::Output, Self::Error> {
            Err(())
        }
    }

    struct PanickingMaterializer;

    impl RowMaterializer<()> for PanickingMaterializer {
        type Output = Entity;
        type Error = std::convert::Infallible;

        fn materialize(
            &self,
            _entity: &mut FilteredEntityAccess<'_, '_>,
            _context: (),
        ) -> Result<Self::Output, Self::Error> {
            panic!("materializer panic")
        }
    }

    fn cache(world: &mut World, mutable: bool, changed: bool) -> CachedQueryCore {
        let component_id = world.register_component::<A>();
        CachedQueryCore::build_auto(
            world,
            &QueryBuildSpec {
                components: vec![QueryComponent {
                    id: component_id,
                    optional: false,
                    mutable,
                }],
                with_filters: Vec::new(),
                without_filters: Vec::new(),
                changed_filters: if changed {
                    vec![component_id]
                } else {
                    Vec::new()
                },
                added_filters: Vec::new(),
                anyof_filters: Vec::new(),
            },
        )
    }

    unsafe fn build_runtime<'a>(
        cache: &'a CachedQueryCore,
        world: &'a mut World,
        validity: ValidityFlag,
    ) -> QueryRuntimeCore {
        let this_run = world.change_tick();
        // SAFETY: forwarded test ownership contract.
        unsafe { build_runtime_with_ticks(cache, world, validity, Tick::new(0), this_run) }
    }

    unsafe fn build_runtime_with_ticks<'a>(
        cache: &'a CachedQueryCore,
        world: &'a mut World,
        validity: ValidityFlag,
        last_run: Tick,
        this_run: Tick,
    ) -> QueryRuntimeCore {
        // SAFETY: the test keeps the cache and world alive until the runtime drops.
        unsafe {
            QueryRuntimeCore::new(
                Some(cache),
                world.as_unsafe_world_cell(),
                validity,
                last_run,
                this_run,
            )
        }
    }

    #[test]
    fn count_empty_and_get_share_one_runtime() {
        let mut world = World::new();
        let first = world.spawn(A(0)).id();
        world.spawn(A(0));
        let cache = cache(&mut world, false, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        assert_eq!(runtime.count().unwrap(), 2);
        assert!(!runtime.is_empty().unwrap());
        assert_eq!(
            runtime
                .get_with(first, &EntityMaterializer, ())
                .unwrap()
                .unwrap(),
            first
        );
        assert!(
            runtime
                .get_with(Entity::from_raw_u32(999).unwrap(), &EntityMaterializer, (),)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn lazy_iteration_restarts_and_rejects_nesting() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, false, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        runtime.begin_iteration().unwrap();
        assert!(
            runtime
                .next_with(&EntityMaterializer, ())
                .unwrap()
                .is_some()
        );
        assert!(matches!(
            runtime.begin_iteration(),
            Err(QueryRuntimeError::NestedIteration)
        ));
        assert!(
            runtime
                .next_with(&EntityMaterializer, ())
                .unwrap()
                .is_none()
        );

        runtime.begin_iteration().unwrap();
        assert!(
            runtime
                .next_with(&EntityMaterializer, ())
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn single_reports_both_cardinality_failures() {
        let mut world = World::new();
        let cache = cache(&mut world, false, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };
        assert!(matches!(
            runtime.single_with(&EntityMaterializer, ()),
            Err(QueryExecutionError::Runtime(QueryRuntimeError::NoEntities))
        ));
        drop(runtime);

        world.spawn(A(0));
        world.spawn(A(0));
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };
        assert!(matches!(
            runtime.single_with(&EntityMaterializer, ()),
            Err(QueryExecutionError::Runtime(
                QueryRuntimeError::MultipleEntities
            ))
        ));
    }

    #[test]
    fn validity_fences_every_operation() {
        let mut world = World::new();
        let entity = world.spawn(A(0)).id();
        let cache = cache(&mut world, false, false);
        let validity = ValidityFlag::new_write();
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, validity.clone()) };
        validity.set_invalid();

        assert!(matches!(
            runtime.check_valid(),
            Err(QueryRuntimeError::Storage(StorageError::InvalidAccess))
        ));
        assert!(matches!(
            runtime.begin_iteration(),
            Err(QueryRuntimeError::Storage(StorageError::InvalidAccess))
        ));
        assert!(matches!(
            runtime.next_with(&EntityMaterializer, ()),
            Err(QueryExecutionError::Runtime(QueryRuntimeError::Storage(
                StorageError::InvalidAccess
            )))
        ));
        assert!(matches!(
            runtime.count(),
            Err(QueryRuntimeError::Storage(StorageError::InvalidAccess))
        ));
        assert!(matches!(
            runtime.is_empty(),
            Err(QueryRuntimeError::Storage(StorageError::InvalidAccess))
        ));
        assert!(matches!(
            runtime.get_with(entity, &EntityMaterializer, ()),
            Err(QueryExecutionError::Runtime(QueryRuntimeError::Storage(
                StorageError::InvalidAccess
            )))
        ));
        assert!(matches!(
            runtime.single_with(&EntityMaterializer, ()),
            Err(QueryExecutionError::Runtime(QueryRuntimeError::Storage(
                StorageError::InvalidAccess
            )))
        ));
        // SAFETY: invalidity is checked before the world cell is accessed.
        assert!(matches!(
            unsafe { runtime.world_ptr() },
            Err(QueryRuntimeError::Storage(StorageError::InvalidAccess))
        ));
    }

    #[test]
    fn mutable_state_yields_variant_aware_access() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, true, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        assert!(
            runtime
                .next_with(&IsMutableMaterializer, ())
                .unwrap()
                .unwrap()
        );
    }

    #[test]
    fn row_materializer_is_the_only_backend_leaf() {
        let mut world = World::new();
        let entity = world.spawn(A(0)).id();
        let cache = cache(&mut world, false, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        assert_eq!(
            runtime.next_with(&EntityMaterializer, ()).unwrap().unwrap(),
            entity
        );
        assert_eq!(
            runtime
                .get_with(entity, &EntityMaterializer, ())
                .unwrap()
                .unwrap(),
            entity
        );
    }

    #[test]
    fn materializer_cannot_reenter_runtime() {
        let mut world = World::new();
        let entity = world.spawn(A(0)).id();
        let cache = cache(&mut world, true, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };
        let materializer = ReentrantMaterializer { runtime: &runtime };

        assert_eq!(
            runtime.next_with(&materializer, ()).unwrap().unwrap(),
            entity
        );
        assert_eq!(runtime.count().unwrap(), 1);
    }

    #[test]
    fn materializer_error_releases_operation_guard() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, true, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        assert!(matches!(
            runtime.next_with(&FailingMaterializer, ()),
            Err(QueryExecutionError::Materialize(()))
        ));
        assert_eq!(runtime.count().unwrap(), 1);
    }

    #[test]
    fn materializer_unwind_releases_operation_guard() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, true, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = runtime.next_with(&PanickingMaterializer, ());
        }));
        assert!(result.is_err());
        assert_eq!(runtime.count().unwrap(), 1);
    }

    #[test]
    fn tick_filters_apply_to_every_lookup_operation() {
        let mut world = World::new();
        let changed = world.spawn(A(0)).id();
        let stale = world.spawn(A(0)).id();
        let cache = cache(&mut world, false, true);

        world.increment_change_tick();
        let last_run = world.change_tick();
        world.increment_change_tick();
        world.entity_mut(changed).get_mut::<A>().unwrap().0 += 1;
        let this_run = world.change_tick();

        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe {
            build_runtime_with_ticks(
                &cache,
                &mut world,
                ValidityFlag::new_write(),
                last_run,
                this_run,
            )
        };

        assert_eq!(runtime.count().unwrap(), 1);
        assert!(!runtime.is_empty().unwrap());
        assert_eq!(
            runtime
                .get_with(changed, &EntityMaterializer, ())
                .unwrap()
                .unwrap(),
            changed
        );
        assert!(
            runtime
                .get_with(stale, &EntityMaterializer, ())
                .unwrap()
                .is_none()
        );
        assert_eq!(
            runtime.single_with(&EntityMaterializer, ()).unwrap(),
            changed
        );
    }
}
