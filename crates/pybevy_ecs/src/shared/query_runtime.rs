//! Interpreter-neutral query iteration and lookup orchestration.
//!
//! [`QueryRuntimeCore`] owns the per-run world cell, tick window, iterator
//! admission, and validity fence. [`IterationToken`] owns each erased iterator.
//! Backends retain only their row materializers:
//! turning a [`FilteredEntityAccess`] into interpreter objects and mapping
//! [`QueryRuntimeError`] into the appropriate Python exception.

use std::{
    fmt,
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use bevy::ecs::{
    change_detection::Tick,
    entity::Entity,
    query::QueryIter,
    world::{FilteredEntityMut, FilteredEntityRef, World, unsafe_world_cell::UnsafeWorldCell},
};
use pybevy_storage::{FilteredEntityAccess, StorageError, ValidityFlag};

use super::{
    cached_query::{CachedQueryCore, ErasedQueryState},
    run_ticks::RunTicks,
};

const NESTED_ITERATION_MESSAGE: &str = "Cannot nest iteration on the same Query (Bevy disallows this via borrow rules). \
     Collect into a list first: items = list(query)";
const REENTRANT_OPERATION_MESSAGE: &str =
    "Cannot re-enter a Query operation while entity access is active";
const ITERATION_IN_PROGRESS_MESSAGE: &str = "Cannot call len()/get()/single()/is_empty() on a Query while iterating it. \
     Collect into a list first: items = list(query)";
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
    /// A fresh-traversal operation (count/is_empty/get/single) was attempted
    /// while a Python `for` loop was paused mid-iteration.
    IterationInProgress,
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
            Self::IterationInProgress => formatter.write_str(ITERATION_IN_PROGRESS_MESSAGE),
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
    // SAFETY: `with_state` selects the variant; the caller guarantees the world
    // and scheduler-access contract and fences the returned iterator with the
    // validity window. Erasing the boxed iterator to `*mut ()` drops the state
    // borrow's lifetime, which the validity fence re-establishes at use.
    unsafe {
        state.with_state(
            |qs| {
                let iterator = qs
                    .query_unchecked_with_ticks(cell, last_run, this_run)
                    .iter_inner();
                ErasedQueryIter::ReadOnly(Box::into_raw(Box::new(iterator)) as *mut ())
            },
            |qs| {
                let iterator = qs
                    .query_unchecked_with_ticks(cell, last_run, this_run)
                    .iter_inner();
                ErasedQueryIter::Mutable(Box::into_raw(Box::new(iterator)) as *mut ())
            },
        )
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
    // SAFETY: `with_state` selects the variant; the caller guarantees the world
    // and scheduler-access contract. The looked-up item borrows only the world
    // (`'a`), not the transient state borrow.
    unsafe {
        state.with_state(
            |qs| {
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .get_inner(entity)
                    .ok()
                    .map(FilteredEntityAccess::Ref)
            },
            |qs| {
                qs.query_unchecked_with_ticks(cell, last_run, this_run)
                    .get_inner(entity)
                    .ok()
                    .map(FilteredEntityAccess::Mut)
            },
        )
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
    validity: ValidityFlag,
    live_iterators: Arc<AtomicUsize>,
    iteration_started: AtomicBool,
    operation_active: AtomicBool,
    last_run: Tick,
    this_run: Tick,
}

/// RAII owner for one Python-level query traversal.
pub struct IterationToken {
    erased: Option<ErasedQueryIter>,
    live: Arc<AtomicUsize>,
}

impl IterationToken {
    /// Release the iterator allocation and live-traversal slot, if held.
    fn release(&mut self) {
        if let Some(iterator) = self.erased.take() {
            // SAFETY: this token uniquely owns the iterator allocation.
            unsafe { iterator.drop_allocation() };
            self.live.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl Drop for IterationToken {
    fn drop(&mut self) {
        self.release();
    }
}

// SAFETY: the token uniquely owns its erased iterator allocation. Backend
// iterator objects serialize advancement, and dropping only frees that owned
// allocation and updates an atomic counter without touching the World.
unsafe impl Send for IterationToken {}
// SAFETY: shared access cannot advance or release the token; both require
// `&mut self`. Backend iterator objects must serialize those mutable operations.
unsafe impl Sync for IterationToken {}

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
            validity,
            live_iterators: Arc::new(AtomicUsize::new(0)),
            iteration_started: AtomicBool::new(false),
            operation_active: AtomicBool::new(false),
            last_run,
            this_run,
        }
    }

    /// Return the shared validity flag used by row materializers.
    pub fn validity(&self) -> &ValidityFlag {
        &self.validity
    }

    /// Return the captured change-detection window for this run.
    pub fn run_ticks(&self) -> RunTicks {
        RunTicks {
            last_run: self.last_run,
            this_run: self.this_run,
        }
    }

    /// Return this runtime's validity-fenced world cell for narrow proxy
    /// write-back operations.
    ///
    /// # Safety
    /// The returned cell must not escape this runtime's validity window, and
    /// callers may access only components covered by the query's declared
    /// scheduler access.
    pub unsafe fn world_cell(&self) -> Result<UnsafeWorldCell<'static>, QueryRuntimeError> {
        self.validity.check()?;
        Ok(self.world_cell)
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
    pub fn begin_iteration(&self) -> Result<IterationToken, QueryRuntimeError> {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        let Some(cached) = self.cached() else {
            return Ok(IterationToken {
                erased: None,
                live: Arc::clone(&self.live_iterators),
            });
        };
        if self
            .live_iterators
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(QueryRuntimeError::NestedIteration);
        }
        self.iteration_started.store(false, Ordering::Release);
        // SAFETY: the constructor binds this cache to this cell, validity was
        // checked above, and scheduler access covers the query.
        let erased =
            unsafe { create_iter(&cached.state, self.world_cell, self.last_run, self.this_run) };
        Ok(IterationToken {
            erased: Some(erased),
            live: Arc::clone(&self.live_iterators),
        })
    }

    /// Advance a traversal token and materialize one row through the backend leaf.
    pub fn advance_with<Context, Materializer>(
        &self,
        token: &mut IterationToken,
        materializer: &Materializer,
        context: Context,
    ) -> Result<Option<Materializer::Output>, QueryExecutionError<Materializer::Error>>
    where
        Materializer: RowMaterializer<Context>,
    {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        let (Some(cached), Some(iterator)) = (self.cached(), token.erased) else {
            return Ok(None);
        };
        self.iteration_started.store(true, Ordering::Release);
        // SAFETY: the token uniquely owns the live allocation and `_operation`
        // prevents overlapping access while the row is materialized.
        match unsafe { self.next_passing(cached, iterator) } {
            Some(mut entity) => materializer
                .materialize(&mut entity, context)
                .map(Some)
                .map_err(QueryExecutionError::Materialize),
            None => {
                token.release();
                Ok(None)
            }
        }
    }

    /// Count matching entities, including Added/Changed filtering.
    pub fn count(&self) -> Result<usize, QueryRuntimeError> {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        self.begin_isolated_operation()?;
        let Some(cached) = self.cached() else {
            return Ok(0);
        };
        if !cached.has_tick_filters() {
            // SAFETY: constructor contract + validity check establish the world/
            // access requirements; `begin_isolated_operation` guaranteed no stored
            // iterator still borrows this state.
            return Ok(unsafe {
                cached
                    .state
                    .count(self.world_cell, self.last_run, self.this_run)
            });
        }

        // SAFETY: as above; the temporary iterator is the only live borrow of the state.
        let iterator =
            unsafe { create_iter(&cached.state, self.world_cell, self.last_run, self.this_run) };
        // The guard ensures the erased allocation is reclaimed on every return.
        let iterator = ErasedIterGuard(iterator);
        let mut count = 0;
        // SAFETY: the guard owns the live iterator for this method scope.
        while unsafe { self.next_passing(cached, iterator.0) }.is_some() {
            count += 1;
        }
        Ok(count)
    }

    /// Return whether no entities match, including Added/Changed filtering.
    pub fn is_empty(&self) -> Result<bool, QueryRuntimeError> {
        self.check_valid()?;
        let _operation = self.enter_operation()?;
        self.begin_isolated_operation()?;
        let Some(cached) = self.cached() else {
            return Ok(true);
        };
        if !cached.has_tick_filters() {
            // SAFETY: as in `count`; no stored iterator still borrows this state.
            return Ok(unsafe {
                cached
                    .state
                    .is_empty_check(self.world_cell, self.last_run, self.this_run)
            });
        }

        // SAFETY: as in `count`; the temporary iterator is the only live borrow.
        let iterator =
            unsafe { create_iter(&cached.state, self.world_cell, self.last_run, self.this_run) };
        let iterator = ErasedIterGuard(iterator);
        // SAFETY: the guard owns the live iterator for this method scope.
        Ok(unsafe { self.next_passing(cached, iterator.0) }.is_none())
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
        self.begin_isolated_operation()?;
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
        // world/cache/access requirements for this temporary iterator; the
        // caller's `begin_isolated_operation` ensured no stored iterator borrows
        // the state.
        let iterator =
            unsafe { create_iter(&cached.state, self.world_cell, self.last_run, self.this_run) };
        let iterator = ErasedIterGuard(iterator);
        // SAFETY: the guard owns the live iterator for this method scope.
        let first = match unsafe { self.next_passing(cached, iterator.0) } {
            Some(access) => access,
            None => return Err(QueryRuntimeError::NoEntities),
        };
        // SAFETY: the guard still owns the live iterator; advancing again probes
        // for a second match (a distinct entity from `first`).
        if unsafe { self.next_passing(cached, iterator.0) }.is_some() {
            return Err(QueryRuntimeError::MultipleEntities);
        }
        Ok(first)
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
        self.begin_isolated_operation()?;
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

    /// Advance `iterator` past entities failing this query's tick filters and
    /// return the next passing entity (or `None` at exhaustion). This is the one
    /// place that pairs `ErasedQueryIter::next` with the per-entity tick check,
    /// so iteration, `count`, `is_empty`, and `single` cannot diverge on which
    /// entities they consider matching.
    ///
    /// # Safety
    ///
    /// The `iterator` allocation must be live and owned by the caller for the
    /// duration of the returned borrow, and validity must already be checked.
    unsafe fn next_passing<'a>(
        &self,
        cached: &CachedQueryCore,
        iterator: ErasedQueryIter,
    ) -> Option<FilteredEntityAccess<'a, 'a>> {
        loop {
            // SAFETY: the caller owns the live allocation and the returned borrow
            // stays within this run; validity was checked upstream.
            match unsafe { iterator.next() } {
                Some(access) if self.passes_tick_filters(cached, &access) => return Some(access),
                Some(_) => {}
                None => return None,
            }
        }
    }

    /// Guard a fresh-traversal operation (`count`/`is_empty`/`get`/`single`)
    /// against a live Python-level iterator that still borrows the QueryState.
    fn begin_isolated_operation(&self) -> Result<(), QueryRuntimeError> {
        if self.live_iterators.load(Ordering::Acquire) != 0 {
            return Err(QueryRuntimeError::IterationInProgress);
        }
        Ok(())
    }

    /// Return whether a live iterator exists but has not advanced yet.
    pub fn has_unadvanced_iterator(&self) -> bool {
        self.live_iterators.load(Ordering::Acquire) != 0
            && !self.iteration_started.load(Ordering::Acquire)
    }

    fn enter_operation(&self) -> Result<OperationGuard<'_>, QueryRuntimeError> {
        if self
            .operation_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(QueryRuntimeError::ReentrantOperation);
        }
        Ok(OperationGuard {
            active: &self.operation_active,
        })
    }
}

// SAFETY: the raw cache/world pointers are fenced by ValidityFlag and scheduler
// access. Cross-object operation and traversal admission use atomics.
unsafe impl Send for QueryRuntimeCore {}

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
    active: &'a AtomicBool,
}

impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
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
            assert!(matches!(
                self.runtime.begin_iteration(),
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
    fn fresh_traversal_rejected_during_paused_iteration() {
        let mut world = World::new();
        let entity = world.spawn(A(0)).id();
        world.spawn(A(0));
        let cache = cache(&mut world, false, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        // Pause a `for` loop after the first entity (iteration is now in progress).
        let mut token = runtime.begin_iteration().unwrap();
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_some()
        );

        // While paused, the fresh-traversal ops must refuse rather than mint a
        // second `&mut QueryState` aliasing the live iterator.
        assert!(matches!(
            runtime.count(),
            Err(QueryRuntimeError::IterationInProgress)
        ));
        assert!(matches!(
            runtime.is_empty(),
            Err(QueryRuntimeError::IterationInProgress)
        ));
        assert!(matches!(
            runtime.get_with(entity, &EntityMaterializer, ()),
            Err(QueryExecutionError::Runtime(
                QueryRuntimeError::IterationInProgress
            ))
        ));
        assert!(matches!(
            runtime.single_with(&EntityMaterializer, ()),
            Err(QueryExecutionError::Runtime(
                QueryRuntimeError::IterationInProgress
            ))
        ));

        // The paused iteration is untouched: it resumes and then exhausts.
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_some()
        );
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_none()
        );

        // After exhaustion the operations work again.
        assert_eq!(runtime.count().unwrap(), 2);
        assert!(!runtime.is_empty().unwrap());
        assert_eq!(
            runtime
                .get_with(entity, &EntityMaterializer, ())
                .unwrap()
                .unwrap(),
            entity
        );
    }

    #[test]
    fn lazy_iteration_restarts_and_rejects_nesting() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, false, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        let mut token = runtime.begin_iteration().unwrap();
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_some()
        );
        assert!(matches!(
            runtime.begin_iteration(),
            Err(QueryRuntimeError::NestedIteration)
        ));
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_none()
        );

        let mut token = runtime.begin_iteration().unwrap();
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
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
            runtime.advance_with(
                &mut IterationToken {
                    erased: None,
                    live: Arc::clone(&runtime.live_iterators),
                },
                &EntityMaterializer,
                (),
            ),
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
        let mut token = runtime.begin_iteration().unwrap();

        assert!(
            runtime
                .advance_with(&mut token, &IsMutableMaterializer, ())
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

        // A lookup and an iteration both materialize through the same leaf. Run
        // the lookup first: probing `get_with` mid-iteration is now rejected.
        assert_eq!(
            runtime
                .get_with(entity, &EntityMaterializer, ())
                .unwrap()
                .unwrap(),
            entity
        );
        assert_eq!(
            runtime
                .advance_with(
                    &mut runtime.begin_iteration().unwrap(),
                    &EntityMaterializer,
                    (),
                )
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
        let mut token = runtime.begin_iteration().unwrap();

        assert_eq!(
            runtime
                .advance_with(&mut token, &materializer, ())
                .unwrap()
                .unwrap(),
            entity
        );
        // Draining the open iteration proves the operation guard was released
        // (no ReentrantOperation) and lets a fresh count run afterwards.
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_none()
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
        let mut token = runtime.begin_iteration().unwrap();

        assert!(matches!(
            runtime.advance_with(&mut token, &FailingMaterializer, ()),
            Err(QueryExecutionError::Materialize(()))
        ));
        // The operation guard released despite the error: the follow-up call is
        // not rejected as re-entrant, and a fresh count then works.
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.count().unwrap(), 1);
    }

    #[test]
    fn materializer_unwind_releases_operation_guard() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, true, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };
        let mut token = runtime.begin_iteration().unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = runtime.advance_with(&mut token, &PanickingMaterializer, ());
        }));
        assert!(result.is_err());
        // The operation guard released on unwind: the follow-up call is not
        // rejected as re-entrant, and a fresh count then works.
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.count().unwrap(), 1);
    }

    #[test]
    fn token_release_is_idempotent_and_drop_releases_slot() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, false, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };

        let mut token = runtime.begin_iteration().unwrap();
        assert_eq!(runtime.live_iterators.load(Ordering::Acquire), 1);
        token.release();
        assert_eq!(runtime.live_iterators.load(Ordering::Acquire), 0);
        token.release();
        assert_eq!(runtime.live_iterators.load(Ordering::Acquire), 0);

        let token = runtime.begin_iteration().unwrap();
        assert_eq!(runtime.live_iterators.load(Ordering::Acquire), 1);
        drop(token);
        assert_eq!(runtime.live_iterators.load(Ordering::Acquire), 0);
    }

    #[test]
    fn exhaustion_releases_slot_while_token_remains_live() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, false, false);
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, ValidityFlag::new_write()) };
        let mut token = runtime.begin_iteration().unwrap();

        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_some()
        );
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.count().unwrap(), 1);
    }

    #[test]
    fn empty_query_token_takes_no_slot() {
        let mut world = World::new();
        let validity = ValidityFlag::new_write();
        let this_run = world.change_tick();
        // SAFETY: world outlives the runtime; there is no cached state pointer.
        let runtime = unsafe {
            QueryRuntimeCore::new(
                None,
                world.as_unsafe_world_cell(),
                validity,
                Tick::new(0),
                this_run,
            )
        };
        let mut token = runtime.begin_iteration().unwrap();

        assert_eq!(runtime.live_iterators.load(Ordering::Acquire), 0);
        assert!(
            runtime
                .advance_with(&mut token, &EntityMaterializer, ())
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.count().unwrap(), 0);
    }

    #[test]
    fn dropping_token_after_invalidation_does_not_touch_world() {
        let mut world = World::new();
        world.spawn(A(0));
        let cache = cache(&mut world, false, false);
        let validity = ValidityFlag::new_write();
        // SAFETY: cache and world outlive `runtime` in this scope.
        let runtime = unsafe { build_runtime(&cache, &mut world, validity.clone()) };
        let token = runtime.begin_iteration().unwrap();

        validity.set_invalid();
        drop(token);
        assert_eq!(runtime.live_iterators.load(Ordering::Acquire), 0);
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
