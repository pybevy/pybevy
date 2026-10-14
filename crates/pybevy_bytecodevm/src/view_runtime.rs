//! Interpreter-neutral safety boundary for View bytecode execution.
//!
//! A [`ValidatedViewProgram`] is an unforgeable capability: it can only be
//! created after the program's declared components, field identities, byte
//! spans, write effects, and VM stack shape have been checked against one
//! resolved View.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem::size_of,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy_ecs::{
    change_detection::Tick,
    component::{ComponentId, Components, StorageType},
    entity::Entity,
    storage::{TableId, TableRow},
    world::{World, WorldId, unsafe_world_cell::UnsafeWorldCell},
};
use nonmax::NonMaxU32;
use pybevy_storage::{StorageError, ValidityFlag};

use crate::{
    bytecode::{
        CompiledBytecode, Compiler, FieldId, FieldType, NegativeIntegerPower, Op, ProgramTooLarge,
        exact_i128,
    },
    expr::RustExpr,
    tiled::{ReduceOp, lane_reduce_slice},
    view_engine::{self, BatchExecutionError, ViewEngineError, ViewFilter},
};

/// The operation for which a bytecode program is being validated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramIntent {
    /// Evaluate a program without writing component memory.
    ReadOnly,
    /// Assign to exactly one declared mutable field.
    Assignment {
        /// The only field the program may store to.
        destination: FieldId,
    },
    /// Evaluate all scalar results before storing one contiguous composite field.
    CompositeAssignment {
        /// First scalar coordinate in one mutable component.
        destination: FieldId,
        /// Number of contiguous f32 coordinates.
        lanes: usize,
    },
}

impl ProgramIntent {
    fn destination_component(self) -> Result<ComponentId, ViewRuntimeError> {
        match self {
            Self::Assignment { destination } => Ok(destination.component_id),
            Self::CompositeAssignment { destination, .. } => Ok(destination.component_id),
            Self::ReadOnly => Err(ViewRuntimeError::ProgramIntentMismatch {
                expected: "assignment",
                actual: "read-only",
            }),
        }
    }
}

/// Interpreter-neutral metadata resolved for one View parameter.
///
/// Construction checks that component layouts, allowed fields, and mutable
/// access all describe the same declared component set. Backend-specific type
/// objects and field names deliberately do not cross this boundary.
#[derive(Debug)]
pub struct ResolvedViewSpec {
    world_id: WorldId,
    filter: ViewFilter,
    mutable_components: HashSet<ComponentId>,
    allowed_fields: HashMap<ComponentId, HashSet<(usize, FieldType)>>,
    component_strides: HashMap<ComponentId, usize>,
}

impl ResolvedViewSpec {
    /// Build a resolved View specification from registration-time metadata.
    ///
    /// # Safety
    ///
    /// - Every component id, stride, and allowed `(offset, FieldType)` must
    ///   describe the real registered layout in `world_id`'s World.
    /// - `mutable_components` and `filter` must be lowered from the same View
    ///   descriptor used to declare this system parameter's scheduler access.
    pub unsafe fn new(
        world_id: WorldId,
        filter: ViewFilter,
        mutable_components: HashSet<ComponentId>,
        mut allowed_fields: HashMap<ComponentId, HashSet<(usize, FieldType)>>,
        component_strides: HashMap<ComponentId, usize>,
    ) -> Result<Self, ViewRuntimeError> {
        for &component_id in &mutable_components {
            if !filter.component_ids.contains(&component_id) {
                return Err(ViewRuntimeError::SpecComponentNotDeclared {
                    component_id,
                    metadata: "mutable component",
                });
            }
        }

        for &component_id in allowed_fields.keys() {
            if !filter.component_ids.contains(&component_id) {
                return Err(ViewRuntimeError::SpecComponentNotDeclared {
                    component_id,
                    metadata: "allowed field",
                });
            }
        }

        for &component_id in component_strides.keys() {
            if !filter.component_ids.contains(&component_id) {
                return Err(ViewRuntimeError::SpecComponentNotDeclared {
                    component_id,
                    metadata: "component stride",
                });
            }
        }

        for &component_id in &filter.component_ids {
            let stride = component_strides
                .get(&component_id)
                .copied()
                .ok_or(ViewRuntimeError::MissingComponentStride(component_id))?;
            let fields = allowed_fields.entry(component_id).or_default();
            for &(offset, field_type) in fields.iter() {
                if matches!(
                    field_type,
                    FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4
                ) {
                    return Err(ViewRuntimeError::CompositeFieldNotExpanded {
                        component_id,
                        offset,
                        field_type,
                    });
                }
                validate_field_span(component_id, offset, field_type, stride)?;
            }
        }

        Ok(Self {
            world_id,
            filter,
            mutable_components,
            allowed_fields,
            component_strides,
        })
    }

    /// World whose component ids and field layouts this specification names.
    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    /// The resolved filter used for exact batch gathering.
    pub fn filter(&self) -> &ViewFilter {
        &self.filter
    }

    /// Components for which this View declared mutable access.
    pub fn mutable_components(&self) -> &HashSet<ComponentId> {
        &self.mutable_components
    }

    /// Legitimate VM-visible field starts and primitive types per component.
    pub fn allowed_fields(&self) -> &HashMap<ComponentId, HashSet<(usize, FieldType)>> {
        &self.allowed_fields
    }

    /// Registered ECS table stride for each declared data component.
    pub fn component_strides(&self) -> &HashMap<ComponentId, usize> {
        &self.component_strides
    }
}

#[derive(Clone, Debug)]
enum CachedProgramKey {
    ReadOnly(RustExpr),
    Assignment {
        destination: FieldId,
        expression: RustExpr,
    },
    CompositeAssignment {
        destination: FieldId,
        expressions: Box<[RustExpr]>,
    },
}

/// A borrowed cache lookup key; the expression is cloned only on a miss.
#[derive(Clone, Copy)]
enum ProgramKey<'a> {
    ReadOnly(&'a RustExpr),
    Assignment {
        destination: FieldId,
        expression: &'a RustExpr,
    },
    CompositeAssignment {
        destination: FieldId,
        expressions: &'a [RustExpr],
    },
}

impl ProgramKey<'_> {
    fn matches(self, cached: &CachedProgramKey) -> bool {
        match (self, cached) {
            (Self::ReadOnly(a), CachedProgramKey::ReadOnly(b)) => a.canonical_eq(b),
            (
                Self::Assignment {
                    destination: ad,
                    expression: ae,
                },
                CachedProgramKey::Assignment {
                    destination: bd,
                    expression: be,
                },
            ) => ad == *bd && ae.canonical_eq(be),
            (
                Self::CompositeAssignment {
                    destination: ad,
                    expressions: ae,
                },
                CachedProgramKey::CompositeAssignment {
                    destination: bd,
                    expressions: be,
                },
            ) => {
                ad == *bd
                    && ae.len() == be.len()
                    && ae.iter().zip(be.iter()).all(|(a, b)| a.canonical_eq(b))
            }
            _ => false,
        }
    }

    fn hash(self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        match self {
            Self::ReadOnly(expression) => {
                0_u8.hash(&mut hasher);
                expression.canonical_hash(&mut hasher);
            }
            Self::CompositeAssignment {
                destination,
                expressions,
            } => {
                2_u8.hash(&mut hasher);
                destination.hash(&mut hasher);
                expressions.len().hash(&mut hasher);
                for expression in expressions {
                    expression.canonical_hash(&mut hasher);
                }
            }
            Self::Assignment {
                destination,
                expression,
            } => {
                1_u8.hash(&mut hasher);
                destination.hash(&mut hasher);
                expression.canonical_hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn into_owned(self) -> CachedProgramKey {
        match self {
            Self::ReadOnly(expression) => CachedProgramKey::ReadOnly(expression.clone()),
            Self::Assignment {
                destination,
                expression,
            } => CachedProgramKey::Assignment {
                destination,
                expression: expression.clone(),
            },
            Self::CompositeAssignment {
                destination,
                expressions,
            } => CachedProgramKey::CompositeAssignment {
                destination,
                expressions: expressions.into(),
            },
        }
    }

    fn compile(self) -> Result<Arc<CompiledBytecode>, ProgramTooLarge> {
        let bytecode = match self {
            Self::ReadOnly(expression) => {
                let mut compiler = Compiler::new();
                expression.compile(&mut compiler);
                compiler.optimize();
                compiler.try_finalize()?
            }
            Self::Assignment {
                destination,
                expression,
            } => view_engine::compile_assignment(
                destination.component_id,
                destination.offset,
                destination.field_type,
                expression,
            )?,
            Self::CompositeAssignment {
                destination,
                expressions,
            } => {
                let mut compiler = Compiler::new();
                for expression in expressions {
                    expression.compile(&mut compiler);
                }
                for lane in (0..expressions.len()).rev() {
                    let index = compiler.add_field(FieldId {
                        offset: destination.offset + lane * size_of::<f32>(),
                        ..destination
                    });
                    compiler.emit(Op::StoreField(index));
                }
                compiler.optimize();
                compiler.try_finalize()?
            }
        };
        Ok(Arc::new(bytecode))
    }
}

#[derive(Debug)]
struct CachedProgram {
    key: CachedProgramKey,
    bytecode: Arc<CompiledBytecode>,
    last_used: u64,
}

/// Upper bound on cached programs per View parameter; expressions embedding a
/// per-frame Python constant produce a distinct program every call.
const PROGRAM_CACHE_CAPACITY: usize = 256;

#[derive(Debug, Default)]
struct ProgramCache {
    buckets: HashMap<u64, Vec<CachedProgram>>,
    len: usize,
    clock: u64,
}

impl ProgramCache {
    fn evict_least_recently_used(&mut self) {
        let Some((&hash, index)) = self
            .buckets
            .iter()
            .flat_map(|(hash, bucket)| {
                bucket
                    .iter()
                    .enumerate()
                    .map(move |(index, cached)| (hash, index, cached.last_used))
            })
            .min_by_key(|&(_, _, last_used)| last_used)
            .map(|(hash, index, _)| (hash, index))
        else {
            return;
        };
        let bucket = self.buckets.get_mut(&hash).expect("bucket exists");
        bucket.swap_remove(index);
        if bucket.is_empty() {
            self.buckets.remove(&hash);
        }
        self.len -= 1;
    }
}

/// Stable, per-parameter View metadata and programs shared across system runs.
#[derive(Debug)]
pub struct CachedViewCore {
    spec: Arc<ResolvedViewSpec>,
    world_id: WorldId,
    programs: Mutex<ProgramCache>,
}

impl CachedViewCore {
    /// Create a stable cache owner after checking live World metadata.
    pub fn new(spec: ResolvedViewSpec, world: &World) -> Result<Self, ViewRuntimeError> {
        validate_spec_world_metadata(&spec, world.id(), world.components())?;
        Ok(Self {
            spec: Arc::new(spec),
            world_id: world.id(),
            programs: Mutex::new(ProgramCache::default()),
        })
    }

    #[cfg(test)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn new_unchecked(spec: ResolvedViewSpec) -> Self {
        let world_id = spec.world_id();
        Self {
            spec: Arc::new(spec),
            world_id,
            programs: Mutex::new(ProgramCache::default()),
        }
    }

    /// Return the exact specification shared by this cache owner.
    pub fn spec(&self) -> &Arc<ResolvedViewSpec> {
        &self.spec
    }

    /// World whose component ids and layouts were used to build this cache.
    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    fn get_or_compile(
        &self,
        key: ProgramKey<'_>,
    ) -> Result<Arc<CompiledBytecode>, ViewRuntimeError> {
        let hash = key.hash();
        self.get_or_compile_with_hash(key, hash)
    }

    fn get_or_compile_with_hash(
        &self,
        key: ProgramKey<'_>,
        hash: u64,
    ) -> Result<Arc<CompiledBytecode>, ViewRuntimeError> {
        let mut cache = self
            .programs
            .lock()
            .map_err(|_| ViewRuntimeError::ProgramCachePoisoned)?;
        cache.clock += 1;
        let clock = cache.clock;
        if let Some(cached) = cache
            .buckets
            .get_mut(&hash)
            .and_then(|bucket| bucket.iter_mut().find(|cached| key.matches(&cached.key)))
        {
            cached.last_used = clock;
            return Ok(Arc::clone(&cached.bytecode));
        }
        let bytecode = key.compile()?;
        if cache.len >= PROGRAM_CACHE_CAPACITY {
            cache.evict_least_recently_used();
        }
        cache.buckets.entry(hash).or_default().push(CachedProgram {
            key: key.into_owned(),
            bytecode: Arc::clone(&bytecode),
            last_used: clock,
        });
        cache.len += 1;
        Ok(bytecode)
    }
}

/// First run-scoped portion of the shared View runtime.
///
/// This initial form is the sole constructor of validated programs. Run-scoped
/// world access and batch leases are added in the subsequent migration slice.
#[derive(Debug)]
pub struct ViewRuntimeCore {
    cached: Arc<CachedViewCore>,
    world_cell: UnsafeWorldCell<'static>,
    validity: ValidityFlag,
    last_run: Tick,
    this_run: Tick,
    operation_active: AtomicBool,
}

impl ViewRuntimeCore {
    /// Create a run-scoped core from stable per-parameter metadata and a World cell.
    ///
    /// `last_run` and `this_run` must be the system invocation's shared tick
    /// window; the runtime never reads or increments the World tick itself.
    ///
    /// # Safety
    ///
    /// - `world_cell` must remain live until `validity` is invalidated and all
    ///   runtime/proxy owners have stopped using it.
    /// - `validity` must belong to this one run and must never be reactivated
    ///   for a later lifetime while any runtime, lease, or proxy owner survives.
    /// - The scheduler access declared for this system parameter must cover all
    ///   data/filter/tick components in `cached.spec()` with the declared
    ///   read/write modes.
    /// - Structural World mutation must not occur while this runtime or any
    ///   lease, slice, or column derived from it can still be used. Leases
    ///   reject a resized or reallocated table, but not rows reordered in place.
    pub unsafe fn new(
        cached: Arc<CachedViewCore>,
        world_cell: UnsafeWorldCell<'_>,
        validity: ValidityFlag,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self, ViewRuntimeError> {
        let actual = world_cell.id();
        if cached.world_id != actual {
            return Err(ViewRuntimeError::WorldMismatch {
                expected: cached.world_id,
                actual,
            });
        }
        // SAFETY: the caller binds this lifetime erasure to `validity` and the
        // system execution window described in the constructor contract.
        let world_cell = unsafe {
            std::mem::transmute::<UnsafeWorldCell<'_>, UnsafeWorldCell<'static>>(world_cell)
        };
        Ok(Self {
            cached,
            world_cell,
            validity,
            last_run,
            this_run,
            operation_active: AtomicBool::new(false),
        })
    }

    /// Return the stable cache owner shared across runs.
    pub fn cached(&self) -> &Arc<CachedViewCore> {
        &self.cached
    }

    /// Return the exact specification owned by this runtime's cache.
    pub fn spec(&self) -> &Arc<ResolvedViewSpec> {
        self.cached.spec()
    }

    /// Return the validity flag shared by this View and every derived proxy.
    pub fn validity(&self) -> &ValidityFlag {
        &self.validity
    }

    /// System tick immediately preceding this invocation.
    pub fn last_run(&self) -> Tick {
        self.last_run
    }

    /// One stable change tick shared by all operations in this invocation.
    pub fn this_run(&self) -> Tick {
        self.this_run
    }

    /// Check that this runtime is live and used on its activating thread.
    pub fn check_valid(&self) -> Result<(), ViewRuntimeError> {
        self.validity.check().map_err(Into::into)
    }

    /// Enter the single operation permitted across this View and all proxies.
    ///
    /// The caller must hold the returned guard through every pointer access and
    /// backend materialization performed by the operation. A second or
    /// re-entrant operation fails closed. Validity is checked once before the
    /// compare-exchange and again after acquisition, immediately before future
    /// execution code may touch run-scoped state.
    pub fn enter_operation(self: &Arc<Self>) -> Result<ViewOperationGuard, ViewRuntimeError> {
        self.check_valid()?;
        self.operation_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| ViewRuntimeError::ReentrantOperation)?;
        let guard = ViewOperationGuard {
            runtime: Arc::clone(self),
            not_send: PhantomData,
        };
        self.check_valid()?;
        Ok(guard)
    }

    /// Gather exact selected table ranges without constructing `&mut World`.
    ///
    /// The returned lease owns the raw batch metadata and re-enters this
    /// runtime's validity/operation fence for every access.
    pub fn gather_batches(self: &Arc<Self>) -> Result<BatchLease, ViewRuntimeError> {
        let _operation = self.enter_operation()?;
        self.validate_live_world_metadata()?;
        // SAFETY: the constructor binds this cell to the cache's World and
        // scheduler access. The operation guard prevents reentrant access, and
        // validity was checked immediately after acquiring it.
        let batches = unsafe {
            view_engine::gather_table_batches_from_cell(
                self.world_cell,
                self.spec().filter(),
                self.last_run,
                self.this_run,
            )
        }?;
        Ok(BatchLease {
            runtime: Arc::clone(self),
            batches: batches.into_boxed_slice(),
        })
    }

    fn validate_live_world_metadata(&self) -> Result<(), ViewRuntimeError> {
        validate_spec_world_metadata(
            self.spec(),
            self.world_cell.id(),
            self.world_cell.components(),
        )
    }

    /// Validate bytecode for one explicit read or assignment operation.
    ///
    /// Validation is deliberately centralized and ordered: instruction-map
    /// integrity, declared components, real field identities, in-layout byte
    /// spans, operation-specific write effects, then complete VM stack shape.
    pub fn validate_program(
        &self,
        bytecode: Arc<CompiledBytecode>,
        intent: ProgramIntent,
    ) -> Result<ValidatedViewProgram, ViewRuntimeError> {
        validate_instruction_indices(&bytecode)?;
        let spec = self.cached.spec();
        view_engine::validate_bytecode_components(&bytecode, &spec.filter.component_ids)?;
        view_engine::validate_bytecode_field_types(&bytecode, &spec.allowed_fields)?;
        validate_bytecode_spans(&bytecode, &spec.component_strides)?;
        validate_write_effects(&bytecode, intent, &spec.mutable_components)?;
        validate_stack_effects(&bytecode, intent)?;
        validate_integer_arithmetic_types(&bytecode)?;

        Ok(ValidatedViewProgram {
            bytecode,
            intent,
            spec: Arc::clone(spec),
        })
    }

    /// Compile/cache and validate one read-only expression for this exact View.
    pub fn prepare_read_program(
        &self,
        expression: &RustExpr,
    ) -> Result<ValidatedViewProgram, ViewRuntimeError> {
        self.check_valid()?;
        let bytecode = self
            .cached
            .get_or_compile(ProgramKey::ReadOnly(expression))?;
        self.validate_program(bytecode, ProgramIntent::ReadOnly)
    }

    /// Compile/cache and validate one assignment for this exact View.
    pub fn prepare_assignment_program(
        &self,
        destination: FieldId,
        expression: &RustExpr,
    ) -> Result<ValidatedViewProgram, ViewRuntimeError> {
        self.check_valid()?;
        let bytecode = self.cached.get_or_compile(ProgramKey::Assignment {
            destination,
            expression,
        })?;
        self.validate_program(bytecode, ProgramIntent::Assignment { destination })
    }

    /// Compile/cache a composite assignment with simultaneous right-hand-side reads.
    pub fn prepare_composite_assignment_program(
        &self,
        destination: FieldId,
        expressions: &[RustExpr],
    ) -> Result<ValidatedViewProgram, ViewRuntimeError> {
        self.check_valid()?;
        let lanes = expressions.len();
        if !valid_composite_destination(destination, lanes) {
            return Err(ViewRuntimeError::InvalidCompositeAssignment);
        }
        let bytecode = self
            .cached
            .get_or_compile(ProgramKey::CompositeAssignment {
                destination,
                expressions,
            })?;
        self.validate_program(
            bytecode,
            ProgramIntent::CompositeAssignment { destination, lanes },
        )
    }

    /// Reject a capability validated for a different View runtime.
    pub fn check_program(&self, program: &ValidatedViewProgram) -> Result<(), ViewRuntimeError> {
        if Arc::ptr_eq(self.cached.spec(), &program.spec) {
            Ok(())
        } else {
            Err(ViewRuntimeError::ProgramFromDifferentRuntime)
        }
    }
}

/// A bytecode program validated for one exact View specification and intent.
///
/// Its fields are private so safe code cannot forge the capability or replace
/// the checked bytecode, intent, or owning specification.
#[derive(Debug)]
pub struct ValidatedViewProgram {
    bytecode: Arc<CompiledBytecode>,
    intent: ProgramIntent,
    spec: Arc<ResolvedViewSpec>,
}

/// RAII proof that one View operation exclusively owns its runtime fence.
///
/// The guard is intentionally neither `Send` nor `Sync`: acquisition, pointer
/// access, and release belong to the system's validity-pinned executor thread.
#[derive(Debug)]
pub struct ViewOperationGuard {
    runtime: Arc<ViewRuntimeCore>,
    not_send: PhantomData<Rc<()>>,
}

impl Drop for ViewOperationGuard {
    fn drop(&mut self) {
        self.runtime
            .operation_active
            .store(false, Ordering::Release);
    }
}

/// Ownership boundary for raw batches gathered during one system run.
///
/// Batches never expose an unchecked safe slice. Backend adapters operate on
/// them only through fenced methods or an owned [`BatchSlice`].
pub struct BatchLease {
    runtime: Arc<ViewRuntimeCore>,
    batches: Box<[view_engine::TableBatch]>,
}

// SAFETY: moving an owner does not permit pointer access. Every safe operation
// rechecks the run-scoped, thread-affine ValidityFlag and acquires the runtime's
// single-operation fence before touching storage. `BatchColumn::raw_ptr_unchecked`
// is unsafe and retains the scheduler/validity obligations at the backend boundary.
unsafe impl Send for BatchLease {}
// SAFETY: shared access is subject to the same validity and operation fences;
// mutable component access is additionally bounded by the resolved View spec.
unsafe impl Sync for BatchLease {}

/// One exact contiguous row run selected from a gathered batch.
///
/// Fields are private so adapters cannot forge a range or combine a range with
/// a different lease. Tick masks are lowered to maximal passing runs before a
/// value of this type is created.
#[derive(Clone)]
pub struct BatchSlice {
    lease: Arc<BatchLease>,
    batch_index: usize,
    start_row: usize,
    entity_count: usize,
}

/// Raw column capability for one exact [`BatchSlice`].
///
/// The capability owns the originating lease. Safe methods expose metadata and
/// fenced validity checks only; dereferencing its pointer remains an explicit
/// backend unsafe boundary.
pub struct BatchColumn {
    lease: Arc<BatchLease>,
    batch_index: usize,
    ptr: *mut u8,
    len: usize,
    stride: usize,
    element_extent: usize,
    writable: bool,
}

// SAFETY: see `BatchLease`. Merely moving or sharing this capability cannot
// dereference its pointer; safe access remains validity/thread/fence checked.
unsafe impl Send for BatchColumn {}
// SAFETY: the scheduler access encoded by the lease excludes conflicting ECS
// access. Backends must uphold the contract of `raw_ptr_unchecked` while native
// code uses the pointer.
unsafe impl Sync for BatchColumn {}

/// Neutral reduction applied to a validated read-only View program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewReduction {
    Sum,
    Min,
    Max,
    CountTruthy,
}

/// Scalar reduction value plus the number of selected rows evaluated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewReductionOutput {
    pub value: f64,
    pub count: usize,
}

impl BatchLease {
    /// Runtime whose World and access declaration own these batches.
    pub fn runtime(&self) -> &Arc<ViewRuntimeCore> {
        &self.runtime
    }

    /// Run a synchronous operation while holding the shared View fence.
    pub fn with_batches<Output>(
        &self,
        operation: impl FnOnce(&[view_engine::TableBatch]) -> Output,
    ) -> Result<Output, ViewRuntimeError> {
        let _operation = self.runtime.enter_operation()?;
        self.runtime.validate_live_world_metadata()?;
        self.validate_batches()?;
        Ok(operation(&self.batches))
    }

    /// Lower gathered tick masks into exact contiguous public batch slices.
    ///
    /// A batch without Changed/Added filters produces one slice. A masked batch
    /// produces one slice for every maximal run of passing rows, so zero-copy
    /// consumers never see or mutate a filtered-out row.
    pub fn contiguous_slices(self: &Arc<Self>) -> Result<Vec<BatchSlice>, ViewRuntimeError> {
        let _operation = self.runtime.enter_operation()?;
        self.runtime.validate_live_world_metadata()?;
        let mut slices = Vec::new();
        for (batch_index, batch) in self.batches.iter().enumerate() {
            self.validate_batch_range(batch)?;
            match &batch.tick_mask {
                None => slices.push(BatchSlice {
                    lease: Arc::clone(self),
                    batch_index,
                    start_row: batch.start_row,
                    entity_count: batch.entity_count,
                }),
                Some(mask) => {
                    if mask.len() != batch.entity_count {
                        return Err(ViewRuntimeError::TickMaskLengthMismatch {
                            table_id: batch.table_id,
                            expected: batch.entity_count,
                            actual: mask.len(),
                        });
                    }
                    let mut local_row = 0;
                    while local_row < mask.len() {
                        while local_row < mask.len() && !mask[local_row] {
                            local_row += 1;
                        }
                        let run_start = local_row;
                        while local_row < mask.len() && mask[local_row] {
                            local_row += 1;
                        }
                        if run_start < local_row {
                            slices.push(BatchSlice {
                                lease: Arc::clone(self),
                                batch_index,
                                start_row: batch.start_row + run_start,
                                entity_count: local_row - run_start,
                            });
                        }
                    }
                }
            }
        }
        Ok(slices)
    }

    /// Count selected rows, including Changed/Added masks.
    pub fn entity_count(&self) -> Result<usize, ViewRuntimeError> {
        self.with_batches(|batches| {
            batches
                .iter()
                .map(|batch| match &batch.tick_mask {
                    Some(mask) => mask.iter().filter(|&&passes| passes).count(),
                    None => batch.entity_count,
                })
                .sum()
        })
    }

    /// Return whether no selected row passes all filters.
    pub fn is_empty(&self) -> Result<bool, ViewRuntimeError> {
        self.entity_count().map(|count| count == 0)
    }

    /// Evaluate one validated read-only program over exact rows.
    pub fn evaluate(
        &self,
        program: &ValidatedViewProgram,
        parallel: bool,
    ) -> Result<Vec<f64>, ViewRuntimeError> {
        self.runtime.check_program(program)?;
        if !matches!(program.intent(), ProgramIntent::ReadOnly) {
            return Err(ViewRuntimeError::ProgramIntentMismatch {
                expected: "read-only",
                actual: "assignment",
            });
        }

        let _operation = self.runtime.enter_operation()?;
        self.runtime.validate_live_world_metadata()?;
        self.validate_batches()?;
        // SAFETY: validation bound every field to this exact spec and the
        // lease owns live selected rows under the operation/validity fences.
        Ok(unsafe {
            view_engine::evaluate_batch_program(
                &self.batches,
                program.bytecode(),
                self.runtime.spec().component_strides(),
                parallel,
            )
        }?)
    }

    /// Evaluate and reduce one validated read-only program over exact rows.
    pub fn reduce(
        &self,
        program: &ValidatedViewProgram,
        reduction: ViewReduction,
        parallel: bool,
    ) -> Result<ViewReductionOutput, ViewRuntimeError> {
        let values = self.evaluate(program, parallel)?;
        let count = values.len();
        // Same accumulator as `pybevy.array`: NaN-propagating, lane-ordered.
        let value = match reduction {
            ViewReduction::Sum => lane_reduce_slice(ReduceOp::Sum, &values),
            ViewReduction::Min => lane_reduce_slice(ReduceOp::Min, &values),
            ViewReduction::Max => lane_reduce_slice(ReduceOp::Max, &values),
            ViewReduction::CountTruthy => {
                values.iter().filter(|value| **value != 0.0).count() as f64
            }
        };
        Ok(ViewReductionOutput { value, count })
    }

    /// Execute one validated assignment over this lease's exact selected rows.
    ///
    /// Destination change ticks are resolved and bounds-checked before the VM
    /// can write. Once execution begins, an unwind-safe marker stamps exactly
    /// those rows with this invocation's stable tick, including after a panic
    /// that may have left a partially written batch.
    pub fn execute_assignment(
        &self,
        program: &ValidatedViewProgram,
        parallel: bool,
    ) -> Result<(), ViewRuntimeError> {
        self.execute_assignment_with(program, |batches, bytecode, strides| {
            let has_tick_masks = batches.iter().any(|batch| batch.tick_mask.is_some());
            // SAFETY: the capability validated every field identity, byte span,
            // source component, and unique mutable destination against this
            // runtime's exact spec. The lease owns live, nonoverlapping selected
            // rows; the operation fence and scheduler access exclude aliases.
            unsafe {
                if has_tick_masks {
                    view_engine::execute_filtered_assignment(batches, bytecode, strides, parallel)
                } else {
                    view_engine::execute_batch_assignment(batches, bytecode, strides, parallel)
                }
            }
        })
    }

    fn execute_assignment_with(
        &self,
        program: &ValidatedViewProgram,
        execute: impl FnOnce(
            &[view_engine::TableBatch],
            &CompiledBytecode,
            &HashMap<ComponentId, usize>,
        ) -> Result<(), BatchExecutionError>,
    ) -> Result<(), ViewRuntimeError> {
        self.runtime.check_program(program)?;
        let destination = program.intent().destination_component()?;

        let _operation = self.runtime.enter_operation()?;
        self.runtime.validate_live_world_metadata()?;
        let changed_ticks = self.resolve_change_ticks(destination)?;
        let _change_marker = ChangeMarkGuard {
            changed_ticks: &changed_ticks,
            this_run: self.runtime.this_run,
        };

        execute(
            &self.batches,
            program.bytecode(),
            self.runtime.spec().component_strides(),
        )?;
        Ok(())
    }

    fn resolve_change_ticks(
        &self,
        destination: ComponentId,
    ) -> Result<Vec<*mut Tick>, ViewRuntimeError> {
        // SAFETY: the runtime constructor binds the cell to this live lease and
        // its scheduler access. The caller holds the shared operation fence and
        // has just rechecked validity and live component metadata.
        let storages = unsafe { self.runtime.world_cell.storages() };
        let mut changed_ticks =
            Vec::with_capacity(self.batches.iter().map(|batch| batch.entity_count).sum());

        for batch in &self.batches {
            let table_entity_count = self.validate_batch_range(batch)?;
            let range_end = batch.start_row + batch.entity_count;
            let table = storages
                .tables
                .get(batch.table_id)
                .ok_or(ViewRuntimeError::BatchTableMissing(batch.table_id))?;
            let column = table.get_column(destination).ok_or(
                ViewRuntimeError::BatchDestinationColumnMissing {
                    table_id: batch.table_id,
                    component_id: destination,
                },
            )?;
            // SAFETY: the column belongs to this live table and the supplied
            // length is its exact current row count. Every selected index is
            // checked against that length before its pointer is retained.
            let table_ticks = unsafe { column.get_changed_ticks_slice(table_entity_count) };

            match &batch.tick_mask {
                Some(mask) => {
                    if mask.len() != batch.entity_count {
                        return Err(ViewRuntimeError::TickMaskLengthMismatch {
                            table_id: batch.table_id,
                            expected: batch.entity_count,
                            actual: mask.len(),
                        });
                    }
                    changed_ticks.extend(
                        mask.iter()
                            .enumerate()
                            .filter(|&(_, passes)| *passes)
                            .map(|(local_row, _)| table_ticks[batch.start_row + local_row].get()),
                    );
                }
                None => {
                    changed_ticks.extend(
                        table_ticks[batch.start_row..range_end]
                            .iter()
                            .map(std::cell::UnsafeCell::get),
                    );
                }
            }
        }
        Ok(changed_ticks)
    }

    fn validate_batches(&self) -> Result<(), ViewRuntimeError> {
        for batch in &self.batches {
            self.validate_batch_range(batch)?;
        }
        Ok(())
    }

    /// Reject a batch whose table changed structurally since it was gathered.
    fn validate_batch_range(
        &self,
        batch: &view_engine::TableBatch,
    ) -> Result<usize, ViewRuntimeError> {
        // SAFETY: callers hold the runtime operation guard and have validated
        // the live World metadata immediately before accessing storage.
        let storages = unsafe { self.runtime.world_cell.storages() };
        let table = storages
            .tables
            .get(batch.table_id)
            .ok_or(ViewRuntimeError::BatchTableMissing(batch.table_id))?;
        let table_entity_count = table.entity_count() as usize;
        let stale = ViewRuntimeError::StaleBatch(batch.table_id);
        if table_entity_count != batch.table_entity_count
            || table_entity_count == 0
            || !std::ptr::eq(table.entities().as_ptr(), batch.entities)
        {
            return Err(stale);
        }
        for (&component_id, &base) in &batch.component_bases {
            // SAFETY: the table is non-empty, so row 0 is in bounds.
            let live = unsafe { table.get_component(component_id, TableRow::new(NonMaxU32::ZERO)) };
            if live.map(|pointer| pointer.as_ptr()) != Some(base) {
                return Err(stale);
            }
        }
        let range_end = batch.start_row.checked_add(batch.entity_count).ok_or(
            ViewRuntimeError::BatchRangeOutOfBounds {
                table_id: batch.table_id,
                start_row: batch.start_row,
                entity_count: batch.entity_count,
                table_entity_count,
            },
        )?;
        if range_end > table_entity_count {
            return Err(ViewRuntimeError::BatchRangeOutOfBounds {
                table_id: batch.table_id,
                start_row: batch.start_row,
                entity_count: batch.entity_count,
                table_entity_count,
            });
        }
        Ok(table_entity_count)
    }
}

impl BatchSlice {
    /// Number of consecutive selected rows in this slice.
    pub fn len(&self) -> usize {
        self.entity_count
    }

    /// Return whether this slice contains no rows.
    pub fn is_empty(&self) -> bool {
        self.entity_count == 0
    }

    /// Narrow this slice to one bounds-checked row.
    pub fn row(&self, index: usize) -> Result<Self, ViewRuntimeError> {
        if index >= self.entity_count {
            return Err(ViewRuntimeError::BatchRowOutOfBounds {
                index,
                len: self.entity_count,
            });
        }
        Ok(Self {
            lease: Arc::clone(&self.lease),
            batch_index: self.batch_index,
            start_row: self.start_row + index,
            entity_count: 1,
        })
    }

    /// Runtime validity check without dereferencing storage.
    pub fn check_valid(&self) -> Result<(), ViewRuntimeError> {
        self.lease.runtime.check_valid()
    }

    /// Copy entity IDs in the same order as the slice's column data.
    pub fn entities(&self) -> Result<Vec<Entity>, ViewRuntimeError> {
        let _operation = self.lease.runtime.enter_operation()?;
        self.lease.runtime.validate_live_world_metadata()?;
        let batch = &self.lease.batches[self.batch_index];
        let table_entity_count = self.lease.validate_batch_range(batch)?;
        let range_end = self.start_row.checked_add(self.entity_count).ok_or(
            ViewRuntimeError::BatchRangeOutOfBounds {
                table_id: batch.table_id,
                start_row: self.start_row,
                entity_count: self.entity_count,
                table_entity_count,
            },
        )?;
        if range_end > table_entity_count {
            return Err(ViewRuntimeError::BatchRangeOutOfBounds {
                table_id: batch.table_id,
                start_row: self.start_row,
                entity_count: self.entity_count,
                table_entity_count,
            });
        }
        // SAFETY: the operation guard, live-metadata check, and scheduler access
        // satisfy UnsafeWorldCell's storage access contract.
        let storages = unsafe { self.lease.runtime.world_cell.storages() };
        let table = storages
            .tables
            .get(batch.table_id)
            .ok_or(ViewRuntimeError::BatchTableMissing(batch.table_id))?;
        Ok(table.entities()[self.start_row..range_end].to_vec())
    }

    /// Evaluate one validated read-only program over only this slice.
    pub fn evaluate(
        &self,
        program: &ValidatedViewProgram,
        parallel: bool,
    ) -> Result<Vec<f64>, ViewRuntimeError> {
        self.lease.runtime.check_program(program)?;
        if !matches!(program.intent(), ProgramIntent::ReadOnly) {
            return Err(ViewRuntimeError::ProgramIntentMismatch {
                expected: "read-only",
                actual: "assignment",
            });
        }
        let _operation = self.lease.runtime.enter_operation()?;
        self.lease.runtime.validate_live_world_metadata()?;
        let batch = self.exact_batch()?;
        // SAFETY: the program is validated for this runtime and `exact_batch`
        // returns one bounds-checked, mask-free selected row range.
        Ok(unsafe {
            view_engine::evaluate_batch_program(
                std::slice::from_ref(&batch),
                program.bytecode(),
                self.lease.runtime.spec().component_strides(),
                parallel,
            )
        }?)
    }

    /// Execute one validated assignment over only this slice.
    pub fn execute_assignment(
        &self,
        program: &ValidatedViewProgram,
        parallel: bool,
    ) -> Result<(), ViewRuntimeError> {
        self.lease.runtime.check_program(program)?;
        let destination = program.intent().destination_component()?;
        // Resolve the destination capability before execution. This validates
        // mutable access and marks exactly these rows changed, matching Bevy's
        // conservative `&mut T` change-detection semantics even if execution
        // later unwinds after a partial write.
        let _destination = self.column(destination, true)?;
        let _operation = self.lease.runtime.enter_operation()?;
        self.lease.runtime.validate_live_world_metadata()?;
        let batch = self.exact_batch()?;
        // SAFETY: program validation proves field identities, spans, and the
        // single mutable destination component; the fence covers execution.
        unsafe {
            view_engine::execute_batch_assignment(
                std::slice::from_ref(&batch),
                program.bytecode(),
                self.lease.runtime.spec().component_strides(),
                parallel,
            )
        }?;
        Ok(())
    }

    fn exact_batch(&self) -> Result<view_engine::TableBatch, ViewRuntimeError> {
        let source = &self.lease.batches[self.batch_index];
        let table_entity_count = self.lease.validate_batch_range(source)?;
        let range_end = self.start_row.checked_add(self.entity_count).ok_or(
            ViewRuntimeError::BatchRangeOutOfBounds {
                table_id: source.table_id,
                start_row: self.start_row,
                entity_count: self.entity_count,
                table_entity_count,
            },
        )?;
        if range_end > table_entity_count {
            return Err(ViewRuntimeError::BatchRangeOutOfBounds {
                table_id: source.table_id,
                start_row: self.start_row,
                entity_count: self.entity_count,
                table_entity_count,
            });
        }
        Ok(view_engine::TableBatch {
            table_id: source.table_id,
            component_bases: source.component_bases.clone(),
            entities: source.entities,
            start_row: self.start_row,
            entity_count: self.entity_count,
            table_entity_count: source.table_entity_count,
            tick_mask: None,
        })
    }

    /// Resolve a raw component column capability for this exact slice.
    ///
    /// Requesting mutable access checks the resolved View declaration and marks
    /// only this slice's rows changed with the invocation's stable `this_run`.
    pub fn column(
        &self,
        component_id: ComponentId,
        mutable: bool,
    ) -> Result<BatchColumn, ViewRuntimeError> {
        let _operation = self.lease.runtime.enter_operation()?;
        self.lease.runtime.validate_live_world_metadata()?;
        let spec = self.lease.runtime.spec();
        if !spec.filter().component_ids.contains(&component_id) {
            return Err(ViewRuntimeError::BatchComponentNotDeclared(component_id));
        }
        if mutable && !spec.mutable_components().contains(&component_id) {
            return Err(ViewRuntimeError::BatchComponentReadOnly(component_id));
        }

        let batch = &self.lease.batches[self.batch_index];
        let table_entity_count = self.lease.validate_batch_range(batch)?;
        let range_end = self.start_row.checked_add(self.entity_count).ok_or(
            ViewRuntimeError::BatchRangeOutOfBounds {
                table_id: batch.table_id,
                start_row: self.start_row,
                entity_count: self.entity_count,
                table_entity_count,
            },
        )?;
        if range_end > table_entity_count {
            return Err(ViewRuntimeError::BatchRangeOutOfBounds {
                table_id: batch.table_id,
                start_row: self.start_row,
                entity_count: self.entity_count,
                table_entity_count,
            });
        }
        let stride = spec
            .component_strides()
            .get(&component_id)
            .copied()
            .ok_or(ViewRuntimeError::MissingComponentStride(component_id))?;
        let base = batch.component_bases.get(&component_id).copied().ok_or(
            ViewRuntimeError::BatchComponentColumnMissing {
                table_id: batch.table_id,
                component_id,
            },
        )?;

        if mutable {
            // SAFETY: the operation guard and scheduler provide unique mutable
            // access to this declared component; all indices are bounds checked.
            let storages = unsafe { self.lease.runtime.world_cell.storages() };
            let table = storages
                .tables
                .get(batch.table_id)
                .ok_or(ViewRuntimeError::BatchTableMissing(batch.table_id))?;
            let column = table.get_column(component_id).ok_or(
                ViewRuntimeError::BatchComponentColumnMissing {
                    table_id: batch.table_id,
                    component_id,
                },
            )?;
            // SAFETY: `table_entity_count` is the live table length.
            let ticks = unsafe { column.get_changed_ticks_slice(table_entity_count) };
            for tick in &ticks[self.start_row..range_end] {
                // SAFETY: each tick belongs to this exact mutable slice.
                unsafe { *tick.get() = self.lease.runtime.this_run() };
            }
        }

        // SAFETY: `start_row * stride` identifies the first byte of this
        // bounds-checked slice within the gathered component allocation.
        let ptr = unsafe { base.add(self.start_row * stride) };
        Ok(BatchColumn {
            lease: Arc::clone(&self.lease),
            batch_index: self.batch_index,
            ptr,
            len: self.entity_count,
            stride,
            element_extent: stride,
            writable: mutable,
        })
    }
}

impl BatchColumn {
    /// Number of elements addressed by this capability.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether this column contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Byte distance between consecutive elements.
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Number of bytes reachable from the column pointer within each element.
    pub fn element_extent(&self) -> usize {
        self.element_extent
    }

    /// Whether this capability came from declared mutable View access.
    pub fn is_writable(&self) -> bool {
        self.writable
    }

    /// Runtime whose validity and operation fence protect this pointer.
    pub fn runtime(&self) -> &Arc<ViewRuntimeCore> {
        self.lease.runtime()
    }

    /// Enter the shared View pointer-access fence after checking that the
    /// column's gathered table has not changed structurally.
    pub fn enter_operation(&self) -> Result<ViewOperationGuard, ViewRuntimeError> {
        let operation = self.lease.runtime.enter_operation()?;
        self.lease.runtime.validate_live_world_metadata()?;
        self.lease
            .validate_batch_range(&self.lease.batches[self.batch_index])?;
        Ok(operation)
    }

    /// Check run lifetime and thread affinity without touching storage.
    pub fn check_valid(&self) -> Result<(), ViewRuntimeError> {
        self.lease.runtime.check_valid()
    }

    /// Derive a bounded field or nested-struct column at `offset` bytes.
    ///
    /// Typed children are restricted to the requested field size. An untyped
    /// child inherits only the bytes remaining after `offset`, so repeated
    /// subcolumn creation can never recover its root component's full stride.
    pub fn subcolumn(
        &self,
        offset: usize,
        field_type: Option<FieldType>,
    ) -> Result<Self, ViewRuntimeError> {
        self.check_valid()?;
        let requested_size = field_type
            .map(|field_type| field_type.size_bytes())
            .unwrap_or_else(|| self.element_extent.saturating_sub(offset));
        let end = offset.checked_add(requested_size);
        if offset >= self.element_extent || end.is_none_or(|end| end > self.element_extent) {
            return Err(ViewRuntimeError::SubcolumnOutOfBounds {
                offset,
                type_size: requested_size,
                element_extent: self.element_extent,
            });
        }

        // SAFETY: the checked span is wholly inside the current element
        // capability. The child retains the same lease, scheduler access,
        // validity fence, row range, and lifetime endpoint as its parent.
        let ptr = unsafe { self.ptr.add(offset) };
        Ok(Self {
            lease: Arc::clone(&self.lease),
            batch_index: self.batch_index,
            ptr,
            len: self.len,
            stride: self.stride,
            element_extent: requested_size,
            writable: self.writable,
        })
    }

    /// Return the first element pointer for an interpreter backend.
    ///
    /// # Safety
    ///
    /// The caller must retain this `BatchColumn`, check validity on the
    /// activating thread immediately before use, and ensure native pointer use
    /// finishes before the system invocation returns. Rust-side dereferences
    /// must additionally hold [`BatchColumn::enter_operation`].
    pub unsafe fn raw_ptr_unchecked(&self) -> *mut u8 {
        self.ptr
    }
}

/// Validate tightly packed bytes before a backend copies them into a View field.
///
/// In addition to enforcing the exact byte length, this rejects non-canonical
/// Rust `bool` representations. Writing any byte other than zero or one into a
/// `bool` field would make later typed Rust access undefined behavior.
pub fn validate_contiguous_field_bytes(
    field_type: FieldType,
    element_count: usize,
    data: &[u8],
) -> Result<(), ViewRuntimeError> {
    let element_size = field_type.size_bytes();
    let expected =
        element_count
            .checked_mul(element_size)
            .ok_or(ViewRuntimeError::BufferSizeOverflow {
                element_count,
                element_size,
            })?;
    if data.len() != expected {
        return Err(ViewRuntimeError::BufferSizeMismatch {
            expected,
            actual: data.len(),
        });
    }
    if field_type == FieldType::Bool
        && let Some((index, value)) = data
            .iter()
            .copied()
            .enumerate()
            .find(|(_, value)| !matches!(value, 0 | 1))
    {
        return Err(ViewRuntimeError::InvalidBooleanBufferValue { index, value });
    }
    Ok(())
}

/// Prevalidated destination ticks armed immediately before possible writes.
///
/// Drop deliberately performs only infallible pointer stores: no allocation,
/// lookup, lock, World access, or backend operation is permitted here.
struct ChangeMarkGuard<'ticks> {
    changed_ticks: &'ticks [*mut Tick],
    this_run: Tick,
}

impl Drop for ChangeMarkGuard<'_> {
    fn drop(&mut self) {
        for &changed_tick in self.changed_ticks {
            // SAFETY: every pointer was obtained from the live destination
            // column for an exact selected row while the runtime's validity and
            // operation fences were active. The scheduler grants unique write
            // access to that destination, and this guard drops before the
            // operation guard releases the fence.
            unsafe {
                changed_tick.write(self.this_run);
            }
        }
    }
}

impl ValidatedViewProgram {
    /// The immutable bytecode that passed validation.
    pub fn bytecode(&self) -> &CompiledBytecode {
        &self.bytecode
    }

    /// The operation for which this program was validated.
    pub fn intent(&self) -> ProgramIntent {
        self.intent
    }
}

/// A Python `int` literal outside the integer lane it meets; NumPy 2 raises
/// `OverflowError` for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegerLiteralOutOfBounds {
    /// The literal's decimal value.
    pub literal: String,
    /// The integer lane it met.
    pub field_type: FieldType,
}

impl fmt::Display for IntegerLiteralOutOfBounds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Python integer {} out of bounds for {}",
            self.literal,
            numpy_dtype_name(self.field_type)
        )
    }
}

/// Failure while resolving a View or validating a program capability.
#[derive(Debug)]
pub enum ViewRuntimeError {
    /// The runtime escaped its system execution window or activating thread.
    Storage(StorageError),
    /// Another operation already owns this runtime's shared pointer-access fence.
    ReentrantOperation,
    /// A panic occurred while the stable compiled-program cache was locked.
    ProgramCachePoisoned,
    /// The expression references more distinct literals or fields than one
    /// compiled program can hold.
    ProgramTooLarge,
    /// Cached component ids were resolved against a different World.
    WorldMismatch {
        /// World used to build the cached specification.
        expected: WorldId,
        /// World presented for this runtime invocation.
        actual: WorldId,
    },
    /// Registration metadata's stride differs from the live component layout.
    ComponentStrideMismatch {
        /// Component with inconsistent layout metadata.
        component_id: ComponentId,
        /// Stride recorded during View resolution.
        resolved: usize,
        /// Stride registered in the runtime's World.
        registered: usize,
    },
    /// One metadata map names a component outside the View's data declaration.
    SpecComponentNotDeclared {
        /// The inconsistent component id.
        component_id: ComponentId,
        /// The metadata category that named it.
        metadata: &'static str,
    },
    /// A declared data component has no resolved table stride.
    MissingComponentStride(ComponentId),
    /// VM-visible vector fields must be expanded into primitive `F32` lanes.
    CompositeFieldNotExpanded {
        /// Component containing the composite field.
        component_id: ComponentId,
        /// Byte offset of the unexpanded field.
        offset: usize,
        /// Composite vector field type.
        field_type: FieldType,
    },
    /// An instruction references a field-map entry that does not exist.
    InvalidFieldIndex {
        /// Instruction that contains the invalid index.
        instruction: &'static str,
        /// Referenced field-map index.
        index: u16,
        /// Number of entries in the field map.
        field_count: usize,
    },
    /// A `PushConst` instruction references a missing constant.
    InvalidConstantIndex {
        /// Referenced constant-pool index.
        index: u16,
        /// Number of entries in the constant pool.
        constant_count: usize,
    },
    /// A read-only operation contains one or more stores.
    StoreInReadOnlyProgram,
    /// An assignment does not contain exactly one store.
    AssignmentStoreCount {
        /// Number of stores found in the bytecode.
        actual: usize,
    },
    /// A composite assignment has invalid destinations or interleaves reads and writes.
    InvalidCompositeAssignment,
    /// The assignment store does not name its declared destination.
    AssignmentDestinationMismatch {
        /// Destination declared by the operation.
        expected: FieldId,
        /// Destination encoded by the bytecode store.
        actual: FieldId,
    },
    /// The assignment destination's component is read-only in this View.
    AssignmentDestinationReadOnly(ComponentId),
    /// A valid capability was presented to a different View runtime.
    ProgramFromDifferentRuntime,
    /// An operation received a capability validated for the wrong intent.
    ProgramIntentMismatch {
        /// Intent required by the operation.
        expected: &'static str,
        /// Intent carried by the capability.
        actual: &'static str,
    },
    /// An instruction requires more stack values than are available.
    StackUnderflow {
        /// Zero-based instruction position.
        instruction_index: usize,
        /// Instruction name.
        instruction: &'static str,
        /// Number of values required by the instruction.
        required: usize,
        /// Number of values present before the instruction.
        available: usize,
    },
    /// An instruction received a value of the wrong VM stack type.
    StackTypeMismatch {
        /// Zero-based instruction position.
        instruction_index: usize,
        /// Instruction name.
        instruction: &'static str,
        /// Stack type required by the instruction.
        expected: &'static str,
        /// Stack type actually present.
        actual: &'static str,
    },
    /// An exact integer-preserving operation combined different integer lane
    /// widths, which the VM would otherwise coerce through f64.
    MixedIntegerArithmetic {
        instruction_index: usize,
        instruction: &'static str,
        field_types: Vec<FieldType>,
    },
    /// A floating result would be silently narrowed into an integer field.
    ImplicitFloatToInteger { destination: FieldType },
    /// A signed integer result would be stored into an unsigned field, which
    /// NumPy's `same_kind` rule rejects.
    ImplicitSignedToUnsigned { destination: FieldType },
    /// An int literal does not fit the integer lane it is combined with or
    /// stored into; NumPy 2 raises `OverflowError` for such Python integers.
    IntegerLiteralOutOfBounds(IntegerLiteralOutOfBounds),
    /// An integer power had a negative exponent, which NumPy rejects. A literal
    /// exponent is rejected before execution; one read from a field is reported
    /// after the single pass, which may already have written rows.
    NegativeIntegerPower,
    /// A program leaves the wrong number of values after execution.
    InvalidFinalStackDepth {
        /// Program intent whose result shape was checked.
        intent: &'static str,
        /// Required final stack depth.
        expected: usize,
        /// Actual final stack depth.
        actual: usize,
    },
    /// A gathered lease references a table that no longer exists.
    BatchTableMissing(TableId),
    /// A gathered table was resized, reallocated, or reordered by a structural
    /// World change after the lease was gathered.
    StaleBatch(TableId),
    /// A gathered range no longer fits its table.
    BatchRangeOutOfBounds {
        /// Table containing the gathered range.
        table_id: TableId,
        /// First selected row.
        start_row: usize,
        /// Number of selected rows.
        entity_count: usize,
        /// Current table row count.
        table_entity_count: usize,
    },
    /// A backend requested a row outside one exact public batch slice.
    BatchRowOutOfBounds {
        /// Requested slice-local row.
        index: usize,
        /// Number of rows in the slice.
        len: usize,
    },
    /// The destination column is absent from a gathered table.
    BatchDestinationColumnMissing {
        /// Table missing the column.
        table_id: TableId,
        /// Destination component.
        component_id: ComponentId,
    },
    /// A batch adapter requested a component outside the View data tuple.
    BatchComponentNotDeclared(ComponentId),
    /// A batch adapter requested mutable access to a read-only component.
    BatchComponentReadOnly(ComponentId),
    /// A declared component column is absent from a gathered table.
    BatchComponentColumnMissing {
        /// Table missing the column.
        table_id: TableId,
        /// Declared component requested by the adapter.
        component_id: ComponentId,
    },
    /// A requested field span escapes its current per-element capability.
    SubcolumnOutOfBounds {
        /// Byte offset relative to the current column pointer.
        offset: usize,
        /// Number of bytes requested at that offset.
        type_size: usize,
        /// Number of bytes reachable in the current element.
        element_extent: usize,
    },
    /// The tightly packed byte length cannot be represented by `usize`.
    BufferSizeOverflow {
        /// Number of selected field elements.
        element_count: usize,
        /// Native byte width of one field element.
        element_size: usize,
    },
    /// A tightly packed write buffer does not exactly cover the selected rows.
    BufferSizeMismatch {
        /// Required byte length.
        expected: usize,
        /// Supplied byte length.
        actual: usize,
    },
    /// A Boolean write buffer contains a byte other than zero or one.
    InvalidBooleanBufferValue {
        /// Zero-based element index containing the invalid byte.
        index: usize,
        /// Invalid byte value.
        value: u8,
    },
    /// A gathered tick mask does not match its row range.
    TickMaskLengthMismatch {
        /// Table whose batch carries the invalid mask.
        table_id: TableId,
        /// Selected row count.
        expected: usize,
        /// Tick-mask length.
        actual: usize,
    },
    /// One of the existing component/field/layout engine checks failed.
    Engine(ViewEngineError),
}

impl fmt::Display for ViewRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => error.fmt(f),
            Self::ReentrantOperation => {
                write!(f, "Cannot re-enter a View operation while access is active")
            }
            Self::ProgramTooLarge => fmt::Display::fmt(&ProgramTooLarge, f),
            Self::ProgramCachePoisoned => {
                write!(
                    f,
                    "View compiled-program cache was poisoned by an earlier panic"
                )
            }
            Self::WorldMismatch { expected, actual } => write!(
                f,
                "Cached View belongs to World {expected:?}, but runtime received World {actual:?}"
            ),
            Self::ComponentStrideMismatch {
                component_id,
                resolved,
                registered,
            } => write!(
                f,
                "Resolved View stride {resolved} for component {component_id:?} does not match \
                 the live World stride {registered}"
            ),
            Self::SpecComponentNotDeclared {
                component_id,
                metadata,
            } => write!(
                f,
                "Resolved View {metadata} metadata references undeclared component {component_id:?}"
            ),
            Self::MissingComponentStride(component_id) => write!(
                f,
                "Resolved View is missing the table stride for component {component_id:?}"
            ),
            Self::CompositeFieldNotExpanded {
                component_id,
                offset,
                field_type,
            } => write!(
                f,
                "Resolved View field at offset {offset} for component {component_id:?} uses \
                 unexpanded composite type {field_type:?}"
            ),
            Self::InvalidFieldIndex {
                instruction,
                index,
                field_count,
            } => write!(
                f,
                "View bytecode {instruction} references field index {index}, but the field map has \
                 {field_count} entries"
            ),
            Self::InvalidConstantIndex {
                index,
                constant_count,
            } => write!(
                f,
                "View bytecode PushConst references constant index {index}, but the constant pool \
                 has {constant_count} entries"
            ),
            Self::StoreInReadOnlyProgram => {
                write!(
                    f,
                    "Read-only View program contains a StoreField instruction"
                )
            }
            Self::AssignmentStoreCount { actual } => write!(
                f,
                "View assignment must contain exactly one StoreField instruction, found {actual}"
            ),
            Self::InvalidCompositeAssignment => write!(
                f,
                "View composite assignment requires 2, 3, 4, 9 or 16 contiguous f32 coordinates with all reads before reversed stores"
            ),
            Self::AssignmentDestinationMismatch { expected, actual } => write!(
                f,
                "View assignment stores to {actual:?}, not its declared destination {expected:?}"
            ),
            Self::AssignmentDestinationReadOnly(component_id) => write!(
                f,
                "View assignment destination component {component_id:?} was not declared mutable"
            ),
            Self::ProgramFromDifferentRuntime => {
                write!(f, "Validated View program belongs to a different runtime")
            }
            Self::ProgramIntentMismatch { expected, actual } => write!(
                f,
                "View operation requires a {expected} program, but received {actual}"
            ),
            Self::StackUnderflow {
                instruction_index,
                instruction,
                required,
                available,
            } => write!(
                f,
                "View bytecode instruction {instruction_index} ({instruction}) needs {required} \
                 stack values, but only {available} are available"
            ),
            Self::StackTypeMismatch {
                instruction_index,
                instruction,
                expected,
                actual,
            } => write!(
                f,
                "View bytecode instruction {instruction_index} ({instruction}) expects {expected}, \
                but found {actual}"
            ),
            Self::MixedIntegerArithmetic {
                instruction_index,
                instruction,
                field_types,
            } => write!(
                f,
                "View bytecode instruction {instruction_index} ({instruction}) mixes integer \
                 field types {field_types:?}; cast fields to one shared width first"
            ),
            Self::ImplicitFloatToInteger { destination } => write!(
                f,
                "Cannot assign a float expression to integer View field {destination:?}"
            ),
            Self::ImplicitSignedToUnsigned { destination } => write!(
                f,
                "Cannot assign a signed integer expression to unsigned View field {destination:?}"
            ),
            Self::IntegerLiteralOutOfBounds(error) => write!(f, "{error}"),
            Self::NegativeIntegerPower => {
                write!(f, "Integers to negative integer powers are not allowed.")
            }
            Self::InvalidFinalStackDepth {
                intent,
                expected,
                actual,
            } => write!(
                f,
                "View {intent} program must leave {expected} stack values, but leaves {actual}"
            ),
            Self::BatchTableMissing(table_id) => {
                write!(f, "Gathered View table {table_id:?} is no longer present")
            }
            Self::StaleBatch(table_id) => write!(
                f,
                "Gathered View table {table_id:?} changed structurally; gather the View again"
            ),
            Self::BatchRangeOutOfBounds {
                table_id,
                start_row,
                entity_count,
                table_entity_count,
            } => write!(
                f,
                "Gathered View range {start_row}..{} is outside table {table_id:?} with \
                 {table_entity_count} rows",
                start_row.saturating_add(*entity_count)
            ),
            Self::BatchRowOutOfBounds { index, len } => {
                write!(
                    f,
                    "View batch row {index} is outside a slice of length {len}"
                )
            }
            Self::BatchDestinationColumnMissing {
                table_id,
                component_id,
            } => write!(
                f,
                "View assignment destination {component_id:?} is absent from table {table_id:?}"
            ),
            Self::BatchComponentNotDeclared(component_id) => write!(
                f,
                "View batch component {component_id:?} is not declared in this View"
            ),
            Self::BatchComponentReadOnly(component_id) => write!(
                f,
                "View batch component {component_id:?} was not declared mutable"
            ),
            Self::BatchComponentColumnMissing {
                table_id,
                component_id,
            } => write!(
                f,
                "View batch component {component_id:?} is absent from table {table_id:?}"
            ),
            Self::SubcolumnOutOfBounds {
                offset,
                type_size,
                element_extent,
            } => write!(
                f,
                "View subcolumn span offset={offset} size={type_size} exceeds element extent \
                 {element_extent}"
            ),
            Self::BufferSizeOverflow {
                element_count,
                element_size,
            } => write!(
                f,
                "View buffer size overflows for {element_count} elements of {element_size} bytes"
            ),
            Self::BufferSizeMismatch { expected, actual } => write!(
                f,
                "Buffer size mismatch: expected {expected} bytes, got {actual}"
            ),
            Self::InvalidBooleanBufferValue { index, value } => write!(
                f,
                "Boolean View buffers accept only bytes 0 or 1; got {value} at index {index}"
            ),
            Self::TickMaskLengthMismatch {
                table_id,
                expected,
                actual,
            } => write!(
                f,
                "View batch for table {table_id:?} has {expected} rows but a tick mask of \
                 length {actual}"
            ),
            Self::Engine(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ViewRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Engine(error) => Some(error),
            _ => None,
        }
    }
}

impl From<StorageError> for ViewRuntimeError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<ProgramTooLarge> for ViewRuntimeError {
    fn from(_: ProgramTooLarge) -> Self {
        Self::ProgramTooLarge
    }
}

impl From<NegativeIntegerPower> for ViewRuntimeError {
    fn from(_: NegativeIntegerPower) -> Self {
        Self::NegativeIntegerPower
    }
}

impl From<BatchExecutionError> for ViewRuntimeError {
    fn from(error: BatchExecutionError) -> Self {
        match error {
            BatchExecutionError::NegativeIntegerPower => Self::NegativeIntegerPower,
            BatchExecutionError::FieldComponentNotGathered(component_id) => {
                ViewEngineError::FieldComponentNotDeclared(component_id).into()
            }
        }
    }
}

impl From<ViewEngineError> for ViewRuntimeError {
    fn from(value: ViewEngineError) -> Self {
        Self::Engine(value)
    }
}

fn numpy_dtype_name(field_type: FieldType) -> &'static str {
    match field_type {
        FieldType::I32 => "int32",
        FieldType::I64 => "int64",
        FieldType::U8 => "uint8",
        FieldType::U32 => "uint32",
        FieldType::U64 => "uint64",
        FieldType::F32 => "float32",
        FieldType::F64 => "float64",
        FieldType::Bool => "bool",
        FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => "vector",
    }
}

fn validate_spec_world_metadata(
    spec: &ResolvedViewSpec,
    actual_world_id: WorldId,
    components: &Components,
) -> Result<(), ViewRuntimeError> {
    if spec.world_id() != actual_world_id {
        return Err(ViewRuntimeError::WorldMismatch {
            expected: spec.world_id(),
            actual: actual_world_id,
        });
    }

    for (&component_id, &resolved) in spec.component_strides() {
        let info = components
            .get_info(component_id)
            .ok_or(ViewEngineError::ComponentNotFound(component_id))?;
        let registered = info.layout().size();
        if resolved != registered {
            return Err(ViewRuntimeError::ComponentStrideMismatch {
                component_id,
                resolved,
                registered,
            });
        }
        if matches!(info.storage_type(), StorageType::SparseSet) {
            return Err(ViewEngineError::SparseDataComponentUnsupported(component_id).into());
        }
    }

    for &component_id in spec
        .filter()
        .changed_ids
        .iter()
        .chain(&spec.filter().added_ids)
    {
        let info = components
            .get_info(component_id)
            .ok_or(ViewEngineError::ComponentNotFound(component_id))?;
        if matches!(info.storage_type(), StorageType::SparseSet) {
            return Err(ViewEngineError::SparseTickComponentUnsupported(component_id).into());
        }
    }
    Ok(())
}

pub(crate) fn validate_instruction_indices(
    bytecode: &CompiledBytecode,
) -> Result<(), ViewRuntimeError> {
    for op in &bytecode.bytecode {
        match *op {
            Op::PushInput(index) => {
                return Err(ViewRuntimeError::InvalidFieldIndex {
                    instruction: "PushInput",
                    index,
                    field_count: 0,
                });
            }
            Op::PushField(index) | Op::StoreField(index)
                if usize::from(index) >= bytecode.field_map.len() =>
            {
                return Err(ViewRuntimeError::InvalidFieldIndex {
                    instruction: match op {
                        Op::PushField(_) => "PushField",
                        Op::StoreField(_) => "StoreField",
                        _ => unreachable!(),
                    },
                    index,
                    field_count: bytecode.field_map.len(),
                });
            }
            Op::PushConst(index) | Op::PushFloatConst(index)
                if usize::from(index) >= bytecode.constants.len() =>
            {
                return Err(ViewRuntimeError::InvalidConstantIndex {
                    index,
                    constant_count: bytecode.constants.len(),
                });
            }
            Op::PushInt(index) if usize::from(index) >= bytecode.integers.len() => {
                return Err(ViewRuntimeError::InvalidConstantIndex {
                    index,
                    constant_count: bytecode.integers.len(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_bytecode_spans(
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
) -> Result<(), ViewRuntimeError> {
    for field in &bytecode.field_map {
        let stride = component_strides
            .get(&field.component_id)
            .copied()
            .ok_or(ViewRuntimeError::MissingComponentStride(field.component_id))?;
        validate_field_span(field.component_id, field.offset, field.field_type, stride)?;
    }
    Ok(())
}

fn validate_field_span(
    component_id: ComponentId,
    offset: usize,
    field_type: FieldType,
    layout_size: usize,
) -> Result<(), ViewRuntimeError> {
    let type_size = field_type.size_bytes();
    let span = offset.checked_add(type_size).ok_or({
        ViewRuntimeError::Engine(ViewEngineError::FieldOffsetOutOfBounds {
            component_id,
            offset,
            type_size,
            layout_size,
        })
    })?;
    if span > layout_size {
        return Err(ViewEngineError::FieldOffsetOutOfBounds {
            component_id,
            offset,
            type_size,
            layout_size,
        }
        .into());
    }
    Ok(())
}

fn valid_composite_destination(destination: FieldId, lanes: usize) -> bool {
    matches!(lanes, 2 | 3 | 4 | 9 | 16)
        && destination.field_type == FieldType::F32
        && destination
            .offset
            .checked_add(lanes * size_of::<f32>())
            .is_some()
}

fn validate_write_effects(
    bytecode: &CompiledBytecode,
    intent: ProgramIntent,
    mutable_components: &HashSet<ComponentId>,
) -> Result<(), ViewRuntimeError> {
    let stores: Vec<FieldId> = bytecode
        .bytecode
        .iter()
        .filter_map(|op| match *op {
            Op::StoreField(index) => Some(bytecode.field_map[usize::from(index)]),
            _ => None,
        })
        .collect();

    match intent {
        ProgramIntent::ReadOnly => {
            if stores.is_empty() {
                Ok(())
            } else {
                Err(ViewRuntimeError::StoreInReadOnlyProgram)
            }
        }
        ProgramIntent::Assignment { destination } => {
            if stores.len() != 1 {
                return Err(ViewRuntimeError::AssignmentStoreCount {
                    actual: stores.len(),
                });
            }
            let actual = stores[0];
            if actual != destination {
                return Err(ViewRuntimeError::AssignmentDestinationMismatch {
                    expected: destination,
                    actual,
                });
            }
            if !mutable_components.contains(&destination.component_id) {
                return Err(ViewRuntimeError::AssignmentDestinationReadOnly(
                    destination.component_id,
                ));
            }
            Ok(())
        }
        ProgramIntent::CompositeAssignment { destination, lanes } => {
            let final_stores = bytecode
                .bytecode
                .iter()
                .rev()
                .take(lanes)
                .all(|op| matches!(op, Op::StoreField(_)));
            if !valid_composite_destination(destination, lanes)
                || stores.len() != lanes
                || !final_stores
                || !stores.iter().enumerate().all(|(index, actual)| {
                    *actual
                        == FieldId {
                            offset: destination.offset + (lanes - 1 - index) * size_of::<f32>(),
                            ..destination
                        }
                })
            {
                return Err(ViewRuntimeError::InvalidCompositeAssignment);
            }
            if !mutable_components.contains(&destination.component_id) {
                return Err(ViewRuntimeError::AssignmentDestinationReadOnly(
                    destination.component_id,
                ));
            }
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum StackKind {
    Float,
    Bool,
}

impl StackKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Bool => "bool",
        }
    }
}

pub(crate) fn validate_stack_effects(
    bytecode: &CompiledBytecode,
    intent: ProgramIntent,
) -> Result<(), ViewRuntimeError> {
    let mut stack = Vec::new();

    for (instruction_index, op) in bytecode.bytecode.iter().enumerate() {
        let (instruction, inputs, output): (&'static str, &[StackKind], Option<StackKind>) =
            match op {
                Op::PushField(index) => {
                    // Bool fields enter the VM as booleans, not floats.
                    let field_type = bytecode.field_map[usize::from(*index)].field_type;
                    stack.push(if field_type == FieldType::Bool {
                        StackKind::Bool
                    } else {
                        StackKind::Float
                    });
                    continue;
                }
                Op::PushInput(index) => {
                    return Err(ViewRuntimeError::InvalidFieldIndex {
                        instruction: "PushInput",
                        index: *index,
                        field_count: 0,
                    });
                }
                Op::PushConst(_) | Op::PushFloatConst(_) | Op::PushInt(_) => {
                    stack.push(StackKind::Float);
                    continue;
                }
                Op::StoreField(index) => {
                    // A Bool destination accepts either kind (the VM stores
                    // whether a number is nonzero); numeric ones require a float.
                    let field_type = bytecode.field_map[usize::from(*index)].field_type;
                    let Some(actual) = stack.pop() else {
                        return Err(ViewRuntimeError::StackUnderflow {
                            instruction_index,
                            instruction: "StoreField",
                            required: 1,
                            available: 0,
                        });
                    };
                    if field_type != FieldType::Bool && actual == StackKind::Bool {
                        return Err(ViewRuntimeError::StackTypeMismatch {
                            instruction_index,
                            instruction: "StoreField",
                            expected: StackKind::Float.name(),
                            actual: actual.name(),
                        });
                    }
                    continue;
                }
                Op::Add => (
                    "Add",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Sub => (
                    "Sub",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Mul => (
                    "Mul",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Div => (
                    "Div",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::FloorDiv => (
                    "FloorDiv",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Pow => (
                    "Pow",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Mod => (
                    "Mod",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Neg => ("Neg", &[StackKind::Float], Some(StackKind::Float)),
                Op::Sin => ("Sin", &[StackKind::Float], Some(StackKind::Float)),
                Op::Cos => ("Cos", &[StackKind::Float], Some(StackKind::Float)),
                Op::Tan => ("Tan", &[StackKind::Float], Some(StackKind::Float)),
                Op::Asin => ("Asin", &[StackKind::Float], Some(StackKind::Float)),
                Op::Acos => ("Acos", &[StackKind::Float], Some(StackKind::Float)),
                Op::Atan => ("Atan", &[StackKind::Float], Some(StackKind::Float)),
                Op::Sqrt => ("Sqrt", &[StackKind::Float], Some(StackKind::Float)),
                Op::Abs => ("Abs", &[StackKind::Float], Some(StackKind::Float)),
                Op::Floor => ("Floor", &[StackKind::Float], Some(StackKind::Float)),
                Op::Ceil => ("Ceil", &[StackKind::Float], Some(StackKind::Float)),
                Op::Round => ("Round", &[StackKind::Float], Some(StackKind::Float)),
                Op::Exp => ("Exp", &[StackKind::Float], Some(StackKind::Float)),
                Op::Ln => ("Ln", &[StackKind::Float], Some(StackKind::Float)),
                Op::Log10 => ("Log10", &[StackKind::Float], Some(StackKind::Float)),
                Op::Log2 => ("Log2", &[StackKind::Float], Some(StackKind::Float)),
                Op::Sign => ("Sign", &[StackKind::Float], Some(StackKind::Float)),
                Op::Fract => ("Fract", &[StackKind::Float], Some(StackKind::Float)),
                Op::Min => (
                    "Min",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Max => (
                    "Max",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Clamp => (
                    "Clamp",
                    &[StackKind::Float, StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Eq => (
                    "Eq",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Bool),
                ),
                Op::Ne => (
                    "Ne",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Bool),
                ),
                Op::Lt => (
                    "Lt",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Bool),
                ),
                Op::Le => (
                    "Le",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Bool),
                ),
                Op::Gt => (
                    "Gt",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Bool),
                ),
                Op::Ge => (
                    "Ge",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Bool),
                ),
                Op::Where => (
                    "Where",
                    &[StackKind::Float, StackKind::Float, StackKind::Bool],
                    Some(StackKind::Float),
                ),
                Op::And => (
                    "And",
                    &[StackKind::Bool, StackKind::Bool],
                    Some(StackKind::Bool),
                ),
                Op::Or => (
                    "Or",
                    &[StackKind::Bool, StackKind::Bool],
                    Some(StackKind::Bool),
                ),
                Op::Not => ("Not", &[StackKind::Bool], Some(StackKind::Bool)),
                Op::Lerp => (
                    "Lerp",
                    &[StackKind::Float, StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
                Op::Random(_) => {
                    stack.push(StackKind::Float);
                    continue;
                }
                Op::RandomRange(_) => (
                    "RandomRange",
                    &[StackKind::Float, StackKind::Float],
                    Some(StackKind::Float),
                ),
            };

        if stack.len() < inputs.len() {
            return Err(ViewRuntimeError::StackUnderflow {
                instruction_index,
                instruction,
                required: inputs.len(),
                available: stack.len(),
            });
        }
        for expected in inputs {
            let actual = stack.pop().expect("length checked above");
            if actual != *expected {
                return Err(ViewRuntimeError::StackTypeMismatch {
                    instruction_index,
                    instruction,
                    expected: expected.name(),
                    actual: actual.name(),
                });
            }
        }
        if let Some(output) = output {
            stack.push(output);
        }
    }

    let (intent_name, expected) = match intent {
        ProgramIntent::ReadOnly => ("read-only", 1),
        ProgramIntent::Assignment { .. } | ProgramIntent::CompositeAssignment { .. } => {
            ("assignment", 0)
        }
    };
    if stack.len() != expected {
        return Err(ViewRuntimeError::InvalidFinalStackDepth {
            intent: intent_name,
            expected,
            actual: stack.len(),
        });
    }
    Ok(())
}

const TYPE_F32: u16 = 1 << 0;
const TYPE_F64: u16 = 1 << 1;
const TYPE_I32: u16 = 1 << 2;
const TYPE_I64: u16 = 1 << 3;
const TYPE_U8: u16 = 1 << 4;
const TYPE_U32: u16 = 1 << 5;
const TYPE_U64: u16 = 1 << 6;
const INTEGER_TYPES: u16 = TYPE_I32 | TYPE_I64 | TYPE_U8 | TYPE_U32 | TYPE_U64;
const SIGNED_TYPES: u16 = TYPE_I32 | TYPE_I64;
const UNSIGNED_TYPES: u16 = TYPE_U8 | TYPE_U32 | TYPE_U64;

#[derive(Clone, Copy)]
enum TrackedValue {
    /// Field-type mask (0 for an int literal) and the value of an int literal.
    Numeric(u16, Option<IntLiteral>),
    Bool,
}

/// The value of a Python `int` literal: exact, or beyond `i128` and held
/// only as its f64 approximation.
#[derive(Clone, Copy)]
enum IntLiteral {
    Exact(i128),
    Huge(f64),
}

impl IntLiteral {
    fn saturated(self) -> i128 {
        match self {
            Self::Exact(value) => value,
            Self::Huge(value) if value > 0.0 => i128::MAX,
            Self::Huge(_) => i128::MIN,
        }
    }

    fn negated(self) -> Self {
        match self {
            Self::Exact(value) => value
                .checked_neg()
                .map_or(Self::Huge(-(value as f64)), Self::Exact),
            Self::Huge(value) => Self::Huge(-value),
        }
    }

    fn out_of_bounds(self, field_type: FieldType) -> ViewRuntimeError {
        let literal = match self {
            Self::Exact(value) => value.to_string(),
            Self::Huge(value) => format!("{value:.0}"),
        };
        ViewRuntimeError::IntegerLiteralOutOfBounds(IntegerLiteralOutOfBounds {
            literal,
            field_type,
        })
    }
}

impl TrackedValue {
    fn numeric(mask: u16) -> Self {
        Self::Numeric(mask, None)
    }
}

fn pop_numeric(stack: &mut Vec<TrackedValue>) -> (u16, Option<IntLiteral>) {
    match stack.pop().expect("stack already validated") {
        TrackedValue::Numeric(mask, literal) => (mask, literal),
        TrackedValue::Bool => unreachable!("numeric operand stack type already validated"),
    }
}

fn field_type_mask(field_type: FieldType) -> u16 {
    match field_type {
        FieldType::F32 => TYPE_F32,
        FieldType::F64 => TYPE_F64,
        FieldType::I32 => TYPE_I32,
        FieldType::I64 => TYPE_I64,
        FieldType::U8 => TYPE_U8,
        FieldType::U32 => TYPE_U32,
        FieldType::U64 => TYPE_U64,
        FieldType::Bool | FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => 0,
    }
}

fn mask_field_types(mask: u16) -> Vec<FieldType> {
    [
        (TYPE_I32, FieldType::I32),
        (TYPE_I64, FieldType::I64),
        (TYPE_U8, FieldType::U8),
        (TYPE_U32, FieldType::U32),
        (TYPE_U64, FieldType::U64),
    ]
    .into_iter()
    .filter_map(|(bit, field_type)| (mask & bit != 0).then_some(field_type))
    .collect()
}

fn preserving_numeric_result(
    left: u16,
    right: u16,
    instruction_index: usize,
    instruction: &'static str,
) -> Result<u16, ViewRuntimeError> {
    let integer_types = (left | right) & INTEGER_TYPES;
    if integer_types.count_ones() > 1 {
        return Err(ViewRuntimeError::MixedIntegerArithmetic {
            instruction_index,
            instruction,
            field_types: mask_field_types(integer_types),
        });
    }
    Ok(match (left, right) {
        (0, value) | (value, 0) => value,
        (left, right) if left == right => left,
        _ => TYPE_F64,
    })
}

/// The field type and the smallest and largest representable values of an
/// integer lane, or `None` for float lanes.
fn integer_lane_range(lane: u16) -> Option<(FieldType, i128, i128)> {
    Some(match lane {
        TYPE_I32 => (FieldType::I32, i32::MIN.into(), i32::MAX.into()),
        TYPE_I64 => (FieldType::I64, i64::MIN.into(), i64::MAX.into()),
        TYPE_U8 => (FieldType::U8, 0, u8::MAX.into()),
        TYPE_U32 => (FieldType::U32, 0, u32::MAX.into()),
        TYPE_U64 => (FieldType::U64, 0, u64::MAX.into()),
        _ => return None,
    })
}

/// Reject an int literal outside the integer lane it meets, as NumPy 2 does
/// for Python integers (NEP 50). Float lanes accept any literal.
fn check_literal_bounds(literal: Option<IntLiteral>, lane: u16) -> Result<(), ViewRuntimeError> {
    match (literal, integer_lane_range(lane)) {
        (Some(literal), Some((field_type, min, max)))
            if !(min..=max).contains(&literal.saturated()) =>
        {
            Err(literal.out_of_bounds(field_type))
        }
        _ => Ok(()),
    }
}

/// NumPy's `clip` accepts an out-of-range bound that cannot bind, and rejects
/// a lower bound above the lane's maximum or an upper bound below its minimum.
fn check_clip_bounds(
    min_literal: Option<IntLiteral>,
    max_literal: Option<IntLiteral>,
    lane: u16,
) -> Result<(), ViewRuntimeError> {
    let Some((field_type, lane_min, lane_max)) = integer_lane_range(lane) else {
        return Ok(());
    };
    match min_literal
        .filter(|literal| literal.saturated() > lane_max)
        .or(max_literal.filter(|literal| literal.saturated() < lane_min))
    {
        Some(literal) => Err(literal.out_of_bounds(field_type)),
        None => Ok(()),
    }
}

/// The casting rule applied when a result is stored into an integer field.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreCasting {
    /// NumPy's `same_kind`, as View assignments apply it: a float or signed
    /// integer result cannot be stored into an unsigned or integer field.
    SameKind,
    /// Cast at the store, as a buffer lens assignment does.
    Unsafe,
}

fn validate_integer_arithmetic_types(bytecode: &CompiledBytecode) -> Result<(), ViewRuntimeError> {
    check_integer_types(bytecode, StoreCasting::SameKind)
}

/// Track primitive field origins through the already stack-valid program and
/// reject only operations whose VM fallback would silently route differing
/// integer widths through f64. Int literals remain polymorphic (`mask == 0`);
/// float literals are f64, so they promote integer operands as in NumPy.
pub(crate) fn check_integer_types(
    bytecode: &CompiledBytecode,
    casting: StoreCasting,
) -> Result<(), ViewRuntimeError> {
    let mut stack: Vec<TrackedValue> = Vec::new();
    for (instruction_index, op) in bytecode.bytecode.iter().enumerate() {
        match *op {
            Op::PushField(index) => {
                let field_type = bytecode.field_map[usize::from(index)].field_type;
                stack.push(if field_type == FieldType::Bool {
                    TrackedValue::Bool
                } else {
                    TrackedValue::numeric(field_type_mask(field_type))
                });
            }
            Op::PushConst(index) => {
                let value = bytecode.constants[usize::from(index)];
                stack.push(match exact_i128(value) {
                    Some(value) => TrackedValue::Numeric(0, Some(IntLiteral::Exact(value))),
                    None if value.is_finite() && value.fract() == 0.0 => {
                        TrackedValue::Numeric(0, Some(IntLiteral::Huge(value)))
                    }
                    None => TrackedValue::numeric(TYPE_F64),
                });
            }
            Op::PushInt(index) => {
                let value = bytecode.integers[usize::from(index)];
                stack.push(TrackedValue::Numeric(0, Some(IntLiteral::Exact(value))));
            }
            Op::PushFloatConst(_) | Op::Random(_) => stack.push(TrackedValue::numeric(TYPE_F64)),
            Op::StoreField(index) => {
                let value = stack.pop().expect("stack already validated");
                let destination = bytecode.field_map[usize::from(index)].field_type;
                if let TrackedValue::Numeric(mask, literal) = value
                    && field_type_mask(destination) & INTEGER_TYPES != 0
                {
                    if casting == StoreCasting::SameKind {
                        if mask & (TYPE_F32 | TYPE_F64) != 0 {
                            return Err(ViewRuntimeError::ImplicitFloatToInteger { destination });
                        }
                        if mask & SIGNED_TYPES != 0
                            && field_type_mask(destination) & UNSIGNED_TYPES != 0
                        {
                            return Err(ViewRuntimeError::ImplicitSignedToUnsigned { destination });
                        }
                    }
                    check_literal_bounds(literal, field_type_mask(destination))?;
                }
            }
            Op::Add | Op::Sub | Op::Mul | Op::Min | Op::Max | Op::FloorDiv | Op::Mod | Op::Pow => {
                let (right, exponent) = pop_numeric(&mut stack);
                let (left, base) = pop_numeric(&mut stack);
                let instruction = match op {
                    Op::Add => "Add",
                    Op::Sub => "Sub",
                    Op::Mul => "Mul",
                    Op::Min => "Min",
                    Op::Max => "Max",
                    Op::FloorDiv => "FloorDiv",
                    Op::Mod => "Mod",
                    Op::Pow => "Pow",
                    _ => unreachable!(),
                };
                let result =
                    preserving_numeric_result(left, right, instruction_index, instruction)?;
                check_literal_bounds(base, result)?;
                check_literal_bounds(exponent, result)?;
                if matches!(op, Op::Pow)
                    && result & (TYPE_F32 | TYPE_F64) == 0
                    && exponent.is_some_and(|literal| literal.saturated() < 0)
                {
                    return Err(ViewRuntimeError::NegativeIntegerPower);
                }
                stack.push(TrackedValue::numeric(result));
            }
            Op::Clamp => {
                let (max, max_literal) = pop_numeric(&mut stack);
                let (min, min_literal) = pop_numeric(&mut stack);
                let (value, _) = pop_numeric(&mut stack);
                let bounds = preserving_numeric_result(min, max, instruction_index, "Clamp")?;
                let result = preserving_numeric_result(value, bounds, instruction_index, "Clamp")?;
                check_clip_bounds(min_literal, max_literal, result)?;
                stack.push(TrackedValue::numeric(result));
            }
            Op::Neg => {
                let (mask, literal) = pop_numeric(&mut stack);
                stack.push(TrackedValue::Numeric(
                    mask,
                    literal.map(IntLiteral::negated),
                ));
            }
            Op::Abs | Op::Floor | Op::Ceil | Op::Round | Op::Sign => {
                let (mask, _) = pop_numeric(&mut stack);
                stack.push(TrackedValue::numeric(mask));
            }
            Op::Div
            | Op::Sin
            | Op::Cos
            | Op::Tan
            | Op::Asin
            | Op::Acos
            | Op::Atan
            | Op::Sqrt
            | Op::Exp
            | Op::Ln
            | Op::Log10
            | Op::Log2
            | Op::Fract
            | Op::Lerp
            | Op::RandomRange(_) => {
                let arity = if matches!(op, Op::Lerp) {
                    3
                } else if matches!(op, Op::Div | Op::RandomRange(_)) {
                    2
                } else {
                    1
                };
                for _ in 0..arity {
                    stack.pop();
                }
                stack.push(TrackedValue::numeric(TYPE_F64));
            }
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                stack.pop();
                stack.pop();
                stack.push(TrackedValue::Bool);
            }
            Op::And | Op::Or => {
                stack.pop();
                stack.pop();
                stack.push(TrackedValue::Bool);
            }
            Op::Not => {}
            Op::Where => {
                let (false_value, _) = pop_numeric(&mut stack);
                let (true_value, _) = pop_numeric(&mut stack);
                stack.pop();
                stack.push(TrackedValue::numeric(true_value | false_value));
            }
            Op::PushInput(_) => unreachable!("instruction indices already validated"),
        }
    }
    Ok(())
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{array::from_fn, ops::Deref, ptr};

    use bevy_ecs::{
        change_detection::DetectChanges, component::Component, entity::Entity, world::World,
    };

    use super::*;
    use crate::tiled::supported_program;

    const STRIDE: usize = 8;

    #[derive(Component)]
    #[repr(transparent)]
    struct RuntimeDense(u32);

    #[derive(Component)]
    #[repr(transparent)]
    struct RuntimeOther(u32);

    #[derive(Component)]
    #[repr(transparent)]
    struct RuntimeWide([u8; 16]);

    #[derive(Component)]
    #[repr(transparent)]
    struct RuntimeVector([f32; 32]);

    #[derive(Component)]
    #[component(storage = "SparseSet")]
    struct RuntimeSparse;

    fn field(component_id: ComponentId, offset: usize, field_type: FieldType) -> FieldId {
        FieldId {
            component_id,
            offset,
            field_type,
        }
    }

    fn filter(component_id: ComponentId) -> ViewFilter {
        ViewFilter {
            component_ids: HashSet::from([component_id]),
            with_ids: Vec::new(),
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        }
    }

    struct TestRuntime {
        // Drop the runtime/cell before freeing the allocation it references.
        runtime: Arc<ViewRuntimeCore>,
        _world: Box<World>,
    }

    struct LiveBatchColumn {
        column: BatchColumn,
        // Drop the column and its run-scoped lease before freeing the World.
        _world: Box<World>,
    }

    impl Deref for TestRuntime {
        type Target = Arc<ViewRuntimeCore>;

        fn deref(&self) -> &Self::Target {
            &self.runtime
        }
    }

    fn runtime(component_id: ComponentId, mutable: bool) -> TestRuntime {
        let mutable_components = if mutable {
            HashSet::from([component_id])
        } else {
            HashSet::new()
        };
        let allowed_fields = HashMap::from([(
            component_id,
            HashSet::from([
                (0, FieldType::F32),
                (4, FieldType::Bool),
                (5, FieldType::Bool),
            ]),
        )]);
        let component_strides = HashMap::from([(component_id, STRIDE)]);
        let mut world = Box::new(World::new());
        // SAFETY: these synthetic layouts are used only by validation tests;
        // no test using this helper gathers or dereferences World storage.
        let spec = unsafe {
            ResolvedViewSpec::new(
                world.id(),
                filter(component_id),
                mutable_components,
                allowed_fields,
                component_strides,
            )
        }
        .unwrap();
        let cached = Arc::new(CachedViewCore::new_unchecked(spec));
        // SAFETY: the boxed World allocation remains stable and is owned by
        // TestRuntime until after its runtime field drops.
        let runtime = unsafe {
            ViewRuntimeCore::new(
                cached,
                world.as_unsafe_world_cell(),
                ValidityFlag::new_write(),
                Tick::new(10),
                Tick::new(20),
            )
        }
        .unwrap();
        TestRuntime {
            runtime: Arc::new(runtime),
            _world: world,
        }
    }

    fn live_batch_column(mutable: bool) -> LiveBatchColumn {
        let mut world = Box::new(World::new());
        world.spawn(RuntimeWide([0; 16]));
        let component_id = world.components().component_id::<RuntimeWide>().unwrap();
        let mutable_components = if mutable {
            HashSet::from([component_id])
        } else {
            HashSet::new()
        };
        let cache = cache_for(
            &world,
            filter(component_id),
            mutable_components,
            HashMap::new(),
        );
        // SAFETY: the boxed World remains live and structurally unchanged until
        // after the returned column releases its lease.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
        let lease = Arc::new(runtime.gather_batches().unwrap());
        let slices = lease.contiguous_slices().unwrap();
        let column = slices[0].column(component_id, mutable).unwrap();
        LiveBatchColumn {
            column,
            _world: world,
        }
    }

    #[test]
    fn contiguous_field_bytes_accept_exact_numeric_payloads() {
        assert!(validate_contiguous_field_bytes(FieldType::F32, 2, &[0xff; 8]).is_ok());
    }

    #[test]
    fn contiguous_field_bytes_accept_canonical_booleans() {
        assert!(validate_contiguous_field_bytes(FieldType::Bool, 4, &[0, 1, 1, 0]).is_ok());
    }

    #[test]
    fn contiguous_field_bytes_reject_noncanonical_boolean() {
        let error = validate_contiguous_field_bytes(FieldType::Bool, 4, &[0, 1, 2, 0])
            .expect_err("non-canonical Rust bool byte must fail closed");

        assert!(matches!(
            error,
            ViewRuntimeError::InvalidBooleanBufferValue { index: 2, value: 2 }
        ));
    }

    #[test]
    fn contiguous_field_bytes_reject_size_mismatch() {
        let error = validate_contiguous_field_bytes(FieldType::F32, 1, &[0; 3])
            .expect_err("partial field payload must fail closed");

        assert!(matches!(
            error,
            ViewRuntimeError::BufferSizeMismatch {
                expected: 4,
                actual: 3
            }
        ));
    }

    #[test]
    fn contiguous_field_bytes_reject_size_overflow() {
        let error = validate_contiguous_field_bytes(FieldType::F64, usize::MAX, &[])
            .expect_err("unrepresentable field buffer size must fail closed");

        assert!(matches!(
            error,
            ViewRuntimeError::BufferSizeOverflow {
                element_count: usize::MAX,
                element_size: 8
            }
        ));
    }

    #[test]
    fn batch_column_root_extent_matches_component_stride() {
        let live = live_batch_column(false);

        assert_eq!(live.column.len(), 1);
        assert_eq!(live.column.stride(), 16);
        assert_eq!(live.column.element_extent(), 16);
        assert!(!live.column.is_writable());
    }

    #[test]
    fn batch_column_typed_subcolumn_preserves_capabilities() {
        let live = live_batch_column(true);
        let child = live.column.subcolumn(8, Some(FieldType::F64)).unwrap();

        assert_eq!(child.len(), live.column.len());
        assert_eq!(child.stride(), live.column.stride());
        assert_eq!(child.element_extent(), FieldType::F64.size_bytes());
        assert!(child.is_writable());
        assert!(Arc::ptr_eq(child.runtime(), live.column.runtime()));
        // SAFETY: both pointers are inspected as addresses only; neither is
        // dereferenced, and the shared live lease remains retained.
        let parent_ptr = unsafe { live.column.raw_ptr_unchecked() };
        // SAFETY: same as above; the shared constructor bounded this offset.
        let child_ptr = unsafe { child.raw_ptr_unchecked() };
        assert_eq!(child_ptr.addr(), parent_ptr.addr() + 8);
    }

    #[test]
    fn batch_column_rejects_typed_span_past_extent() {
        let live = live_batch_column(false);

        assert!(matches!(
            live.column.subcolumn(12, Some(FieldType::F64)),
            Err(ViewRuntimeError::SubcolumnOutOfBounds {
                offset: 12,
                type_size: 8,
                element_extent: 16,
            })
        ));
        assert!(matches!(
            live.column.subcolumn(usize::MAX, Some(FieldType::F32)),
            Err(ViewRuntimeError::SubcolumnOutOfBounds {
                offset: usize::MAX,
                type_size: 4,
                element_extent: 16,
            })
        ));
    }

    #[test]
    fn batch_column_nested_subcolumns_keep_remaining_extent() {
        let live = live_batch_column(false);
        let nested = live.column.subcolumn(12, None).unwrap();

        assert_eq!(nested.element_extent(), 4);
        assert!(nested.subcolumn(0, Some(FieldType::F32)).is_ok());
        assert!(matches!(
            nested.subcolumn(0, Some(FieldType::F64)),
            Err(ViewRuntimeError::SubcolumnOutOfBounds {
                offset: 0,
                type_size: 8,
                element_extent: 4,
            })
        ));
        assert!(matches!(
            nested.subcolumn(4, Some(FieldType::F32)),
            Err(ViewRuntimeError::SubcolumnOutOfBounds {
                offset: 4,
                type_size: 4,
                element_extent: 4,
            })
        ));
    }

    fn stale_batch_world(mutable: bool) -> (Box<World>, Arc<BatchLease>, ComponentId) {
        let mut world = Box::new(World::new());
        world.spawn(RuntimeDense(1));
        world.spawn(RuntimeDense(2));
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let mutable_components = if mutable {
            HashSet::from([component_id])
        } else {
            HashSet::new()
        };
        let cache = cache_for(
            &world,
            filter(component_id),
            mutable_components,
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        // SAFETY: the World stays boxed and live; these tests mutate it
        // structurally on purpose and assert that no stale pointer is used.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
        let lease = Arc::new(runtime.gather_batches().unwrap());
        (world, lease, component_id)
    }

    fn is_stale<T>(result: Result<T, ViewRuntimeError>) -> bool {
        matches!(result, Err(ViewRuntimeError::StaleBatch(_)))
    }

    #[test]
    fn lease_rejects_table_reallocated_after_gather() {
        let (mut world, lease, component_id) = stale_batch_world(true);
        let slices = lease.contiguous_slices().unwrap();
        let column = slices[0].column(component_id, true).unwrap();
        let program = lease
            .runtime()
            .prepare_read_program(&RustExpr::Field {
                component_id,
                offset: 0,
                field_type: FieldType::U32,
            })
            .unwrap();

        for value in 0..10_000 {
            world.spawn(RuntimeDense(value));
        }

        assert!(is_stale(column.enter_operation()));
        assert!(is_stale(slices[0].column(component_id, false)));
        assert!(is_stale(slices[0].entities()));
        assert!(is_stale(slices[0].evaluate(&program, false)));
        assert!(is_stale(lease.entity_count()));
        assert!(is_stale(lease.evaluate(&program, false)));
        assert!(is_stale(lease.contiguous_slices()));
        lease.runtime().validity().set_invalid();
    }

    #[test]
    fn lease_rejects_table_shrunk_after_gather() {
        let (mut world, lease, component_id) = stale_batch_world(true);
        let slices = lease.contiguous_slices().unwrap();
        let first = slices[0].entities().unwrap()[0];
        let destination = field(component_id, 0, FieldType::U32);
        let program = lease
            .runtime()
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();

        world.despawn(first);

        assert!(is_stale(lease.execute_assignment(&program, false)));
        assert!(is_stale(slices[0].execute_assignment(&program, false)));
        lease.runtime().validity().set_invalid();
        drop(slices);
        drop(lease);
        let values: Vec<u32> = world
            .query::<&RuntimeDense>()
            .iter(&world)
            .map(|dense| dense.0)
            .collect();
        assert!(
            !values.contains(&42),
            "no stale assignment may land: {values:?}"
        );
    }

    #[test]
    fn batch_column_subcolumn_rejects_stale_runtime() {
        let live = live_batch_column(false);
        live.column.runtime().validity().set_invalid();

        assert!(matches!(
            live.column.subcolumn(0, Some(FieldType::F32)),
            Err(ViewRuntimeError::Storage(StorageError::InvalidAccess))
        ));
    }

    fn read_program(field: FieldId) -> Arc<CompiledBytecode> {
        Arc::new(CompiledBytecode {
            bytecode: vec![Op::PushField(0)],
            constants: Vec::new(),
            integers: Vec::new(),
            field_map: vec![field],
        })
    }

    fn vector_cache(
        world: &World,
        component_id: ComponentId,
        mutable: bool,
    ) -> Arc<CachedViewCore> {
        cache_for(
            world,
            filter(component_id),
            if mutable {
                HashSet::from([component_id])
            } else {
                HashSet::new()
            },
            HashMap::from([(
                component_id,
                (0..32).map(|lane| (lane * 4, FieldType::F32)).collect(),
            )]),
        )
    }

    #[test]
    fn composite_assignment_snapshots_rhs_on_every_execution_path() {
        for lanes in [2, 3, 4, 9, 16] {
            for parallel in [false, true] {
                for sliced in [false, true] {
                    for fallback in [false, true] {
                        let mut world = World::new();
                        let entities: Vec<_> = (0..513)
                            .map(|index| {
                                world
                                    .spawn(RuntimeVector(from_fn(|lane| {
                                        index as f32 + lane as f32 + 2.0
                                    })))
                                    .id()
                            })
                            .collect();
                        let component_id =
                            world.components().component_id::<RuntimeVector>().unwrap();
                        let cache = vector_cache(&world, component_id, true);
                        let this_run = Tick::new(77);
                        // SAFETY: the World stays live and structurally unchanged until invalidation.
                        let runtime =
                            unsafe { runtime_for_world(&mut world, cache, Tick::new(0), this_run) };
                        let destination = field(component_id, 0, FieldType::F32);
                        let expressions: Vec<_> = (0..lanes)
                            .map(|lane| {
                                let expression = RustExpr::Mul(
                                    Box::new(RustExpr::Field {
                                        component_id,
                                        offset: 0,
                                        field_type: FieldType::F32,
                                    }),
                                    Box::new(RustExpr::Field {
                                        component_id,
                                        offset: lane * 4,
                                        field_type: FieldType::F32,
                                    }),
                                );
                                if fallback && lane == 0 {
                                    RustExpr::Add(
                                        Box::new(expression),
                                        Box::new(RustExpr::Random(0)),
                                    )
                                } else {
                                    expression
                                }
                            })
                            .collect();
                        let program = runtime
                            .prepare_composite_assignment_program(destination, &expressions)
                            .unwrap();
                        assert_eq!(supported_program(program.bytecode()), !fallback);
                        let lease = Arc::new(runtime.gather_batches().unwrap());
                        let expected: Vec<_> = expressions
                            .iter()
                            .map(|expression| {
                                lease
                                    .evaluate(
                                        &runtime.prepare_read_program(expression).unwrap(),
                                        false,
                                    )
                                    .unwrap()
                            })
                            .collect();
                        if sliced {
                            for slice in lease.contiguous_slices().unwrap() {
                                slice.execute_assignment(&program, parallel).unwrap();
                            }
                        } else {
                            lease.execute_assignment(&program, parallel).unwrap();
                        }
                        runtime.validity().set_invalid();
                        drop(lease);
                        drop(runtime);
                        for (row, entity) in entities.iter().enumerate() {
                            let entity_ref = world.entity(*entity);
                            let vector = entity_ref.get_ref::<RuntimeVector>().unwrap();
                            assert_eq!(
                                &vector.0[..lanes],
                                &expected
                                    .iter()
                                    .map(|axis| axis[row] as f32)
                                    .collect::<Vec<_>>()
                            );
                            assert_eq!(vector.last_changed(), this_run);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn composite_assignment_validation_rejects_read_only_and_interleaved_stores() {
        let mut world = World::new();
        world.spawn(RuntimeVector([1.0; 32]));
        let component_id = world.components().component_id::<RuntimeVector>().unwrap();
        for mutable in [false, true] {
            let cache = vector_cache(&world, component_id, mutable);
            // SAFETY: the World remains live and unchanged; this test only validates programs.
            let runtime =
                unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
            let destinations = [0, 4, 8].map(|offset| field(component_id, offset, FieldType::F32));
            let expressions = [
                RustExpr::Const(2.0),
                RustExpr::Const(3.0),
                RustExpr::Const(4.0),
            ];
            for count in [0, 1, 5, 17] {
                assert!(matches!(
                    runtime.prepare_composite_assignment_program(
                        destinations[0],
                        &vec![RustExpr::Const(1.0); count]
                    ),
                    Err(ViewRuntimeError::InvalidCompositeAssignment)
                ));
            }
            let overflow = field(component_id, usize::MAX - 3, FieldType::F32);
            assert!(matches!(
                runtime.prepare_composite_assignment_program(overflow, &expressions),
                Err(ViewRuntimeError::InvalidCompositeAssignment)
            ));
            let result =
                runtime.prepare_composite_assignment_program(destinations[0], &expressions);
            if mutable {
                let program = result.unwrap();
                let mut interleaved = program.bytecode().clone();
                interleaved.bytecode.swap(1, 3);
                assert!(matches!(
                    runtime.validate_program(Arc::new(interleaved), program.intent()),
                    Err(ViewRuntimeError::InvalidCompositeAssignment)
                ));
                for indices in [[0, 0, 0], [0, 2, 1]] {
                    let mut forged = program.bytecode().clone();
                    for (op, index) in forged.bytecode.iter_mut().rev().take(3).zip(indices) {
                        *op = Op::StoreField(index);
                    }
                    assert!(matches!(
                        runtime.validate_program(Arc::new(forged), program.intent()),
                        Err(ViewRuntimeError::InvalidCompositeAssignment)
                    ));
                }
            } else {
                assert!(
                    matches!(result, Err(ViewRuntimeError::AssignmentDestinationReadOnly(id)) if id == component_id)
                );
            }
            runtime.validity().set_invalid();
        }
    }

    #[test]
    fn composite_assignment_cache_distinguishes_each_axis_and_destination() {
        let mut world = World::new();
        world.spawn(RuntimeVector([1.0; 32]));
        let component_id = world.components().component_id::<RuntimeVector>().unwrap();
        let cache = vector_cache(&world, component_id, true);
        // SAFETY: the World remains live and unchanged; only program preparation is tested.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
        let destinations = [0, 4, 8].map(|offset| field(component_id, offset, FieldType::F32));
        let expressions = [
            RustExpr::Const(2.0),
            RustExpr::Const(3.0),
            RustExpr::Const(4.0),
        ];
        let first = runtime
            .prepare_composite_assignment_program(destinations[0], &expressions)
            .unwrap();
        let again = runtime
            .prepare_composite_assignment_program(destinations[0], &expressions)
            .unwrap();
        assert!(ptr::eq(first.bytecode(), again.bytecode()));
        for axis in 0..3 {
            let mut changed = expressions.clone();
            changed[axis] = RustExpr::Const(5.0);
            let next = runtime
                .prepare_composite_assignment_program(destinations[0], &changed)
                .unwrap();
            assert!(!ptr::eq(first.bytecode(), next.bytecode()));
            let cache = runtime.cached();
            let first_collision = cache
                .get_or_compile_with_hash(
                    ProgramKey::CompositeAssignment {
                        destination: destinations[0],
                        expressions: &expressions,
                    },
                    17,
                )
                .unwrap();
            let next_collision = cache
                .get_or_compile_with_hash(
                    ProgramKey::CompositeAssignment {
                        destination: destinations[0],
                        expressions: &changed,
                    },
                    17,
                )
                .unwrap();
            assert!(!Arc::ptr_eq(&first_collision, &next_collision));
        }
        let short = runtime
            .prepare_composite_assignment_program(destinations[0], &expressions[..2])
            .unwrap();
        assert!(!ptr::eq(first.bytecode(), short.bytecode()));
        let cache = runtime.cached();
        let long_collision = cache
            .get_or_compile_with_hash(
                ProgramKey::CompositeAssignment {
                    destination: destinations[0],
                    expressions: &expressions,
                },
                17,
            )
            .unwrap();
        let short_collision = cache
            .get_or_compile_with_hash(
                ProgramKey::CompositeAssignment {
                    destination: destinations[0],
                    expressions: &expressions[..2],
                },
                17,
            )
            .unwrap();
        assert!(!Arc::ptr_eq(&long_collision, &short_collision));
        let other_destinations = destinations.map(|mut destination| {
            destination.offset += 12;
            destination
        });
        let other = runtime
            .prepare_composite_assignment_program(other_destinations[0], &expressions)
            .unwrap();
        assert!(!ptr::eq(first.bytecode(), other.bytecode()));
        runtime.validity().set_invalid();
    }

    fn assignment_program(destination: FieldId) -> Arc<CompiledBytecode> {
        Arc::new(CompiledBytecode {
            bytecode: vec![Op::PushConst(0), Op::StoreField(0)],
            constants: vec![42.0],
            integers: Vec::new(),
            field_map: vec![destination],
        })
    }

    fn dense_value_and_tick(world: &World, entity: Entity) -> (u32, Tick) {
        let entity_ref = world.entity(entity);
        let value = entity_ref.get_ref::<RuntimeDense>().unwrap();
        (value.0, value.last_changed())
    }

    fn cache_for(
        world: &World,
        filter: ViewFilter,
        mutable_components: HashSet<ComponentId>,
        allowed_fields: HashMap<ComponentId, HashSet<(usize, FieldType)>>,
    ) -> Arc<CachedViewCore> {
        let component_strides = filter
            .component_ids
            .iter()
            .map(|&component_id| {
                let stride = world
                    .components()
                    .get_info(component_id)
                    .unwrap()
                    .layout()
                    .size();
                (component_id, stride)
            })
            .collect();
        // SAFETY: callers derive field metadata from the concrete test
        // components declared above; strides come from this exact World.
        let spec = unsafe {
            ResolvedViewSpec::new(
                world.id(),
                filter,
                mutable_components,
                allowed_fields,
                component_strides,
            )
        }
        .unwrap();
        Arc::new(CachedViewCore::new(spec, world).unwrap())
    }

    fn synthetic_spec(
        filter: ViewFilter,
        mutable_components: HashSet<ComponentId>,
        allowed_fields: HashMap<ComponentId, HashSet<(usize, FieldType)>>,
        component_strides: HashMap<ComponentId, usize>,
    ) -> Result<ResolvedViewSpec, ViewRuntimeError> {
        // SAFETY: these specs exercise constructor validation only and are
        // never used to gather or execute against a World.
        unsafe {
            ResolvedViewSpec::new(
                World::new().id(),
                filter,
                mutable_components,
                allowed_fields,
                component_strides,
            )
        }
    }

    unsafe fn runtime_for_world(
        world: &mut World,
        cached: Arc<CachedViewCore>,
        last_run: Tick,
        this_run: Tick,
    ) -> Arc<ViewRuntimeCore> {
        // SAFETY: forwarded from the caller; tests keep the World allocation
        // live and structurally unchanged through every runtime operation.
        let runtime = unsafe {
            ViewRuntimeCore::new(
                cached,
                world.as_unsafe_world_cell(),
                ValidityFlag::new_write(),
                last_run,
                this_run,
            )
        }
        .unwrap();
        Arc::new(runtime)
    }

    #[test]
    fn resolved_spec_normalizes_empty_allowed_field_sets() {
        let component_id = ComponentId::new(1);
        let spec = synthetic_spec(
            filter(component_id),
            HashSet::new(),
            HashMap::new(),
            HashMap::from([(component_id, 0)]),
        )
        .unwrap();

        assert_eq!(spec.allowed_fields()[&component_id], HashSet::new());
    }

    #[test]
    fn resolved_spec_rejects_mutability_for_undeclared_component() {
        let declared = ComponentId::new(1);
        let forged = ComponentId::new(2);
        let result = synthetic_spec(
            filter(declared),
            HashSet::from([forged]),
            HashMap::new(),
            HashMap::from([(declared, STRIDE)]),
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::SpecComponentNotDeclared {
                component_id,
                metadata: "mutable component",
            }) if component_id == forged
        ));
    }

    #[test]
    fn resolved_spec_rejects_fields_for_undeclared_component() {
        let declared = ComponentId::new(1);
        let forged = ComponentId::new(2);
        let result = synthetic_spec(
            filter(declared),
            HashSet::new(),
            HashMap::from([(forged, HashSet::from([(0, FieldType::F32)]))]),
            HashMap::from([(declared, STRIDE)]),
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::SpecComponentNotDeclared {
                component_id,
                metadata: "allowed field",
            }) if component_id == forged
        ));
    }

    #[test]
    fn resolved_spec_requires_exact_declared_stride_set() {
        let declared = ComponentId::new(1);
        let extra = ComponentId::new(2);

        let missing = synthetic_spec(
            filter(declared),
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(matches!(
            missing,
            Err(ViewRuntimeError::MissingComponentStride(id)) if id == declared
        ));

        let extra_result = synthetic_spec(
            filter(declared),
            HashSet::new(),
            HashMap::new(),
            HashMap::from([(declared, STRIDE), (extra, STRIDE)]),
        );
        assert!(matches!(
            extra_result,
            Err(ViewRuntimeError::SpecComponentNotDeclared {
                component_id,
                metadata: "component stride",
            }) if component_id == extra
        ));
    }

    #[test]
    fn resolved_spec_rejects_out_of_bounds_and_overflowing_fields() {
        let component_id = ComponentId::new(1);
        for offset in [STRIDE, usize::MAX] {
            let result = synthetic_spec(
                filter(component_id),
                HashSet::new(),
                HashMap::from([(component_id, HashSet::from([(offset, FieldType::F32)]))]),
                HashMap::from([(component_id, STRIDE)]),
            );
            assert!(matches!(
                result,
                Err(ViewRuntimeError::Engine(
                    ViewEngineError::FieldOffsetOutOfBounds { .. }
                ))
            ));
        }
    }

    #[test]
    fn resolved_spec_rejects_unexpanded_vector_fields() {
        let component_id = ComponentId::new(1);
        let result = synthetic_spec(
            filter(component_id),
            HashSet::new(),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::Vec2)]))]),
            HashMap::from([(component_id, STRIDE)]),
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::CompositeFieldNotExpanded {
                component_id: id,
                offset: 0,
                field_type: FieldType::Vec2,
            }) if id == component_id
        ));
    }

    #[test]
    fn read_only_validation_accepts_declared_real_field() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, false);
        let bytecode = read_program(field(component_id, 0, FieldType::F32));

        let program = runtime
            .validate_program(Arc::clone(&bytecode), ProgramIntent::ReadOnly)
            .unwrap();

        assert!(std::ptr::eq(program.bytecode(), bytecode.as_ref()));
        assert_eq!(program.intent(), ProgramIntent::ReadOnly);
        assert!(runtime.check_program(&program).is_ok());
    }

    #[test]
    fn assignment_validation_accepts_one_exact_mutable_store() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 0, FieldType::F32);
        let runtime = runtime(component_id, true);

        let program = runtime
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();

        assert_eq!(program.intent(), ProgramIntent::Assignment { destination });
    }

    #[test]
    fn read_only_validation_rejects_forged_store() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, true);
        let result = runtime.validate_program(
            assignment_program(field(component_id, 0, FieldType::F32)),
            ProgramIntent::ReadOnly,
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::StoreInReadOnlyProgram)
        ));
    }

    #[test]
    fn assignment_validation_requires_exactly_one_store() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 0, FieldType::F32);
        let runtime = runtime(component_id, true);

        let no_store = runtime.validate_program(
            read_program(destination),
            ProgramIntent::Assignment { destination },
        );
        assert!(matches!(
            no_store,
            Err(ViewRuntimeError::AssignmentStoreCount { actual: 0 })
        ));

        let two_stores = Arc::new(CompiledBytecode {
            bytecode: vec![
                Op::PushConst(0),
                Op::StoreField(0),
                Op::PushConst(0),
                Op::StoreField(0),
            ],
            constants: vec![1.0],
            integers: Vec::new(),
            field_map: vec![destination],
        });
        let result =
            runtime.validate_program(two_stores, ProgramIntent::Assignment { destination });
        assert!(matches!(
            result,
            Err(ViewRuntimeError::AssignmentStoreCount { actual: 2 })
        ));
    }

    #[test]
    fn assignment_validation_rejects_different_destination() {
        let component_id = ComponentId::new(1);
        let declared = field(component_id, 0, FieldType::F32);
        let actual = field(component_id, 4, FieldType::Bool);
        let runtime = runtime(component_id, true);

        let result = runtime.validate_program(
            assignment_program(actual),
            ProgramIntent::Assignment {
                destination: declared,
            },
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::AssignmentDestinationMismatch {
                expected,
                actual: encoded,
            }) if expected == declared && encoded == actual
        ));
    }

    #[test]
    fn assignment_validation_rejects_read_only_destination() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 0, FieldType::F32);
        let runtime = runtime(component_id, false);

        let result = runtime.validate_program(
            assignment_program(destination),
            ProgramIntent::Assignment { destination },
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::AssignmentDestinationReadOnly(id)) if id == component_id
        ));
    }

    #[test]
    fn validation_rejects_undeclared_component_before_field_lookup() {
        let declared = ComponentId::new(1);
        let forged = ComponentId::new(2);
        let runtime = runtime(declared, false);

        let result = runtime.validate_program(
            read_program(field(forged, 0, FieldType::F32)),
            ProgramIntent::ReadOnly,
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::Engine(
                ViewEngineError::FieldComponentNotDeclared(id)
            )) if id == forged
        ));
    }

    #[test]
    fn validation_rejects_type_confusion_and_mid_field_offsets() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, false);

        for forged in [
            field(component_id, 0, FieldType::Bool),
            field(component_id, 2, FieldType::F32),
        ] {
            let result = runtime.validate_program(read_program(forged), ProgramIntent::ReadOnly);
            assert!(matches!(
                result,
                Err(ViewRuntimeError::Engine(
                    ViewEngineError::FieldTypeMismatch { .. }
                ))
            ));
        }
    }

    #[test]
    fn validation_rejects_invalid_instruction_indices() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, true);

        for (op, instruction) in [
            (Op::PushField(1), "PushField"),
            (Op::StoreField(1), "StoreField"),
        ] {
            let result = runtime.validate_program(
                Arc::new(CompiledBytecode {
                    bytecode: vec![op],
                    constants: Vec::new(),
                    integers: Vec::new(),
                    field_map: vec![field(component_id, 0, FieldType::F32)],
                }),
                ProgramIntent::ReadOnly,
            );
            assert!(matches!(
                result,
                Err(ViewRuntimeError::InvalidFieldIndex {
                    instruction: actual,
                    index: 1,
                    field_count: 1,
                }) if actual == instruction
            ));
        }

        let result = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::PushConst(0)],
                constants: Vec::new(),
                integers: Vec::new(),
                field_map: Vec::new(),
            }),
            ProgramIntent::ReadOnly,
        );
        assert!(matches!(
            result,
            Err(ViewRuntimeError::InvalidConstantIndex {
                index: 0,
                constant_count: 0,
            })
        ));

        let result = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::PushInput(0)],
                constants: Vec::new(),
                integers: Vec::new(),
                field_map: Vec::new(),
            }),
            ProgramIntent::ReadOnly,
        );
        assert!(matches!(
            result,
            Err(ViewRuntimeError::InvalidFieldIndex {
                instruction: "PushInput",
                index: 0,
                field_count: 0,
            })
        ));
    }

    #[test]
    fn validation_rejects_stack_underflow_type_mismatch_and_extra_results() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 0, FieldType::F32);
        let runtime = runtime(component_id, true);

        let underflow = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::StoreField(0)],
                constants: Vec::new(),
                integers: Vec::new(),
                field_map: vec![destination],
            }),
            ProgramIntent::Assignment { destination },
        );
        assert!(matches!(
            underflow,
            Err(ViewRuntimeError::StackUnderflow {
                instruction_index: 0,
                instruction: "StoreField",
                required: 1,
                available: 0,
            })
        ));

        let wrong_type = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::PushConst(0), Op::Not],
                constants: vec![1.0],
                integers: Vec::new(),
                field_map: Vec::new(),
            }),
            ProgramIntent::ReadOnly,
        );
        assert!(matches!(
            wrong_type,
            Err(ViewRuntimeError::StackTypeMismatch {
                instruction_index: 1,
                instruction: "Not",
                expected: "bool",
                actual: "float",
            })
        ));

        let extra_result = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::PushConst(0), Op::StoreField(0), Op::PushConst(0)],
                constants: vec![1.0],
                integers: Vec::new(),
                field_map: vec![destination],
            }),
            ProgramIntent::Assignment { destination },
        );
        assert!(matches!(
            extra_result,
            Err(ViewRuntimeError::InvalidFinalStackDepth {
                intent: "assignment",
                expected: 0,
                actual: 1,
            })
        ));
    }

    #[test]
    fn validation_accepts_boolean_result_stack_shape() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, false);
        let result = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::PushConst(0), Op::PushConst(1), Op::Lt, Op::Not],
                constants: vec![1.0, 2.0],
                integers: Vec::new(),
                field_map: Vec::new(),
            }),
            ProgramIntent::ReadOnly,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn validated_program_is_shared_across_runs_of_same_cache() {
        let component_id = ComponentId::new(1);
        let first = runtime(component_id, false);
        // SAFETY: the second runtime shares the first TestRuntime's live World
        // cell and cache, and is dropped before that TestRuntime.
        let second = unsafe {
            ViewRuntimeCore::new(
                Arc::clone(first.cached()),
                first.runtime.world_cell,
                ValidityFlag::new_write(),
                Tick::new(20),
                Tick::new(30),
            )
        }
        .map(Arc::new)
        .unwrap();
        let program = first
            .validate_program(
                read_program(field(component_id, 0, FieldType::F32)),
                ProgramIntent::ReadOnly,
            )
            .unwrap();

        assert!(first.check_program(&program).is_ok());
        assert!(second.check_program(&program).is_ok());
    }

    #[test]
    fn prepared_program_cache_reuses_exact_expression_across_runs() {
        let component_id = ComponentId::new(1);
        let first = runtime(component_id, false);
        // SAFETY: the second runtime shares the first TestRuntime's live World
        // cell/cache and is dropped before the owning TestRuntime.
        let second = unsafe {
            ViewRuntimeCore::new(
                Arc::clone(first.cached()),
                first.runtime.world_cell,
                ValidityFlag::new_write(),
                Tick::new(20),
                Tick::new(30),
            )
        }
        .map(Arc::new)
        .unwrap();
        let expression = RustExpr::Const(7.0);

        let first_program = first.prepare_read_program(&expression).unwrap();
        let second_program = second.prepare_read_program(&expression).unwrap();

        assert!(std::ptr::eq(
            first_program.bytecode(),
            second_program.bytecode()
        ));
    }

    #[test]
    fn prepared_program_cache_resolves_hash_collisions_by_full_key() {
        let runtime = runtime(ComponentId::new(1), false);
        let cache = runtime.cached();
        let forced_hash = 17;

        let first = cache
            .get_or_compile_with_hash(ProgramKey::ReadOnly(&RustExpr::Const(1.0)), forced_hash)
            .unwrap();
        let second = cache
            .get_or_compile_with_hash(ProgramKey::ReadOnly(&RustExpr::Const(2.0)), forced_hash)
            .unwrap();
        let first_again = cache
            .get_or_compile_with_hash(ProgramKey::ReadOnly(&RustExpr::Const(1.0)), forced_hash)
            .unwrap();

        assert_eq!(first.constants, vec![1.0]);
        assert_eq!(second.constants, vec![2.0]);
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(&first, &first_again));
    }

    #[test]
    fn program_cache_stays_bounded_under_varying_constants() {
        let runtime = runtime(ComponentId::new(1), false);
        let cache = runtime.cached();

        // A per-frame varying constant produces a distinct program each call.
        for frame in 0..(PROGRAM_CACHE_CAPACITY * 2) {
            cache
                .get_or_compile(ProgramKey::ReadOnly(&RustExpr::Const(frame as f64)))
                .unwrap();
        }

        let programs = cache.programs.lock().unwrap();
        assert_eq!(programs.len, PROGRAM_CACHE_CAPACITY);
        assert_eq!(
            programs
                .buckets
                .values()
                .map(|bucket| bucket.len())
                .sum::<usize>(),
            PROGRAM_CACHE_CAPACITY
        );
    }

    #[test]
    fn program_cache_eviction_keeps_recently_used_programs() {
        let runtime = runtime(ComponentId::new(1), false);
        let cache = runtime.cached();
        let stable = RustExpr::Const(-1.0);

        let first = cache.get_or_compile(ProgramKey::ReadOnly(&stable)).unwrap();
        for frame in 0..(PROGRAM_CACHE_CAPACITY * 2) {
            cache
                .get_or_compile(ProgramKey::ReadOnly(&RustExpr::Const(frame as f64)))
                .unwrap();
            // Touch the stable program so churn evicts one-shot entries instead.
            let again = cache.get_or_compile(ProgramKey::ReadOnly(&stable)).unwrap();
            assert!(Arc::ptr_eq(&first, &again));
        }
    }

    #[test]
    fn poisoned_program_cache_fails_closed() {
        let runtime = runtime(ComponentId::new(1), false);
        let cache = Arc::clone(runtime.cached());
        let result = std::panic::catch_unwind(move || {
            let _guard = cache.programs.lock().unwrap();
            panic!("poison View program cache");
        });
        assert!(result.is_err());

        assert!(matches!(
            runtime.prepare_read_program(&RustExpr::Const(1.0)),
            Err(ViewRuntimeError::ProgramCachePoisoned)
        ));
    }

    #[test]
    fn validated_program_is_rejected_by_equivalent_but_distinct_cache() {
        let component_id = ComponentId::new(1);
        let first = runtime(component_id, false);
        let second = runtime(component_id, false);
        let program = first
            .validate_program(
                read_program(field(component_id, 0, FieldType::F32)),
                ProgramIntent::ReadOnly,
            )
            .unwrap();

        assert!(matches!(
            second.check_program(&program),
            Err(ViewRuntimeError::ProgramFromDifferentRuntime)
        ));
    }

    #[test]
    fn runtime_preserves_shared_tick_window() {
        let runtime = runtime(ComponentId::new(1), false);

        assert_eq!(runtime.last_run(), Tick::new(10));
        assert_eq!(runtime.this_run(), Tick::new(20));
    }

    #[test]
    fn stale_validity_rejects_operation_before_access() {
        let runtime = runtime(ComponentId::new(1), false);
        runtime.validity().set_invalid();

        assert!(matches!(
            runtime.check_valid(),
            Err(ViewRuntimeError::Storage(StorageError::InvalidAccess))
        ));
        assert!(matches!(
            runtime.enter_operation(),
            Err(ViewRuntimeError::Storage(StorageError::InvalidAccess))
        ));
        assert!(matches!(
            runtime.prepare_read_program(&RustExpr::Const(1.0)),
            Err(ViewRuntimeError::Storage(StorageError::InvalidAccess))
        ));
    }

    #[test]
    fn cross_thread_operation_is_rejected_before_fence_acquisition() {
        let runtime = runtime(ComponentId::new(1), false);
        let other = Arc::clone(&runtime.runtime);

        let rejected = std::thread::spawn(move || {
            matches!(
                other.enter_operation(),
                Err(ViewRuntimeError::Storage(StorageError::CrossThreadAccess))
            )
        })
        .join()
        .unwrap();

        assert!(rejected);
        assert!(runtime.enter_operation().is_ok());
    }

    #[test]
    fn operation_fence_is_shared_by_all_runtime_owners() {
        let runtime = runtime(ComponentId::new(1), true);
        let proxy_owner = Arc::clone(&runtime.runtime);
        let guard = runtime.enter_operation().unwrap();

        assert!(matches!(
            proxy_owner.enter_operation(),
            Err(ViewRuntimeError::ReentrantOperation)
        ));

        drop(guard);
        assert!(proxy_owner.enter_operation().is_ok());
    }

    #[test]
    fn operation_guard_releases_after_unwind() {
        let runtime = runtime(ComponentId::new(1), true);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = runtime.enter_operation().unwrap();
            panic!("injected View operation panic");
        }));

        assert!(result.is_err());
        assert!(runtime.enter_operation().is_ok());
    }

    #[test]
    fn runtime_rejects_cache_from_different_world() {
        let mut first = World::new();
        let component_id = first.register_component::<RuntimeDense>();
        let cache = cache_for(
            &first,
            filter(component_id),
            HashSet::new(),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        let mut second = World::new();

        // SAFETY: the second World is live for this constructor call. The
        // constructor rejects its identity before storing the cell.
        let result = unsafe {
            ViewRuntimeCore::new(
                cache,
                second.as_unsafe_world_cell(),
                ValidityFlag::new_write(),
                Tick::new(0),
                Tick::new(1),
            )
        };

        assert!(matches!(
            result,
            Err(ViewRuntimeError::WorldMismatch { .. })
        ));
    }

    #[test]
    fn gather_rejects_resolved_stride_that_differs_from_live_world() {
        let mut world = World::new();
        let component_id = world.register_component::<RuntimeDense>();
        // SAFETY: this intentionally violates the constructor contract to prove
        // the live-World defense rejects forged registration metadata before
        // any table pointer is gathered.
        let forged = unsafe {
            ResolvedViewSpec::new(
                world.id(),
                filter(component_id),
                HashSet::new(),
                HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
                HashMap::from([(component_id, STRIDE)]),
            )
        }
        .unwrap();
        let cache = Arc::new(CachedViewCore::new_unchecked(forged));
        // SAFETY: the World remains live and unchanged through the operation.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };

        assert!(matches!(
            runtime.gather_batches(),
            Err(ViewRuntimeError::ComponentStrideMismatch {
                component_id: id,
                resolved: STRIDE,
                registered,
            }) if id == component_id && registered == std::mem::size_of::<RuntimeDense>()
        ));
    }

    #[test]
    fn batch_lease_preserves_exact_sparse_range_and_validity() {
        let mut world = World::new();
        world.spawn(RuntimeDense(10));
        world.spawn((RuntimeDense(20), RuntimeSparse));
        world.spawn((RuntimeDense(30), RuntimeSparse));
        world.spawn(RuntimeDense(40));
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let marker_id = world.components().component_id::<RuntimeSparse>().unwrap();
        let view_filter = ViewFilter {
            component_ids: HashSet::from([component_id]),
            with_ids: vec![marker_id],
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        };
        let cache = cache_for(
            &world,
            view_filter,
            HashSet::new(),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        // SAFETY: the World remains live and structurally unchanged until the
        // lease and runtime are dropped below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
        let lease = runtime.gather_batches().unwrap();

        assert_eq!(lease.entity_count().unwrap(), 2);
        let values = lease
            .with_batches(|batches| {
                assert_eq!(batches.len(), 1);
                assert_eq!(batches[0].start_row, 1);
                assert_eq!(batches[0].entity_count, 2);
                (0..batches[0].entity_count)
                    .map(|local_row| {
                        let table_row = batches[0].start_row + local_row;
                        let base = batches[0].component_bases[&component_id];
                        // SAFETY: the batch was gathered from this live World;
                        // `table_row` is inside its exact selected range and the
                        // pointer names the RuntimeDense table column.
                        unsafe { (*base.cast::<RuntimeDense>().add(table_row)).0 }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(values, vec![20, 30]);

        let program = runtime
            .prepare_read_program(&RustExpr::Field {
                component_id,
                offset: 0,
                field_type: FieldType::U32,
            })
            .unwrap();

        runtime.validity().set_invalid();
        assert!(matches!(
            lease.entity_count(),
            Err(ViewRuntimeError::Storage(StorageError::InvalidAccess))
        ));
        assert!(matches!(
            lease.reduce(&program, ViewReduction::Sum, false),
            Err(ViewRuntimeError::Storage(StorageError::InvalidAccess))
        ));
    }

    #[test]
    fn cache_rejects_sparse_data_even_without_entities() {
        let mut world = World::new();
        let component_id = world.register_component::<RuntimeSparse>();
        // SAFETY: the empty allowed-field set and registered zero-sized stride
        // exactly describe RuntimeSparse; cache validation rejects its storage.
        let spec = unsafe {
            ResolvedViewSpec::new(
                world.id(),
                filter(component_id),
                HashSet::new(),
                HashMap::new(),
                HashMap::from([(component_id, 0)]),
            )
        }
        .unwrap();

        assert!(matches!(
            CachedViewCore::new(spec, &world),
            Err(ViewRuntimeError::Engine(
                ViewEngineError::SparseDataComponentUnsupported(id)
            )) if id == component_id
        ));
    }

    #[test]
    fn cache_rejects_sparse_tick_source_even_without_entities() {
        let mut world = World::new();
        let component_id = world.register_component::<RuntimeDense>();
        let marker_id = world.register_component::<RuntimeSparse>();
        let mut view_filter = filter(component_id);
        view_filter.changed_ids.push(marker_id);
        // SAFETY: the data component metadata exactly describes RuntimeDense;
        // cache validation rejects the sparse tick source independently.
        let spec = unsafe {
            ResolvedViewSpec::new(
                world.id(),
                view_filter,
                HashSet::new(),
                HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
                HashMap::from([(component_id, std::mem::size_of::<RuntimeDense>())]),
            )
        }
        .unwrap();

        assert!(matches!(
            CachedViewCore::new(spec, &world),
            Err(ViewRuntimeError::Engine(
                ViewEngineError::SparseTickComponentUnsupported(id)
            )) if id == marker_id
        ));
    }

    #[test]
    fn core_gather_applies_dense_added_tick_mask() {
        let mut world = World::new();
        world.spawn(RuntimeDense(10));
        world.clear_trackers();
        let last_run = world.last_change_tick();
        world.increment_change_tick();
        world.spawn(RuntimeDense(20));
        let this_run = world.change_tick();
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let mut view_filter = filter(component_id);
        view_filter.added_ids.push(component_id);
        let cache = cache_for(
            &world,
            view_filter,
            HashSet::new(),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        // SAFETY: the World remains live and structurally unchanged through the
        // lease operations.
        let runtime = unsafe { runtime_for_world(&mut world, cache, last_run, this_run) };
        let lease = runtime.gather_batches().unwrap();

        assert_eq!(lease.entity_count().unwrap(), 1);
        let selected = lease
            .with_batches(|batches| {
                batches
                    .iter()
                    .flat_map(|batch| {
                        batch
                            .tick_mask
                            .as_ref()
                            .unwrap()
                            .iter()
                            .enumerate()
                            .filter(|&(_, passes)| *passes)
                            .map(|(local_row, _)| {
                                let table_row = batch.start_row + local_row;
                                let base = batch.component_bases[&component_id];
                                // SAFETY: the passing row is inside this live
                                // batch's exact dense-table range.
                                unsafe { (*base.cast::<RuntimeDense>().add(table_row)).0 }
                            })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(selected, vec![20]);

        let expression = RustExpr::Field {
            component_id,
            offset: 0,
            field_type: FieldType::U32,
        };
        let program = runtime.prepare_read_program(&expression).unwrap();
        let output = lease.reduce(&program, ViewReduction::Sum, true).unwrap();
        assert_eq!(
            output,
            ViewReductionOutput {
                value: 20.0,
                count: 1
            }
        );
    }

    #[test]
    fn contiguous_slices_split_noncontiguous_changed_mask() {
        let mut world = World::new();
        let first = world.spawn(RuntimeDense(10)).id();
        world.spawn(RuntimeDense(20));
        let third = world.spawn(RuntimeDense(30)).id();
        world.clear_trackers();
        let last_run = world.last_change_tick();
        world.increment_change_tick();
        world.entity_mut(first).get_mut::<RuntimeDense>().unwrap().0 = 11;
        world.entity_mut(third).get_mut::<RuntimeDense>().unwrap().0 = 31;
        let this_run = world.change_tick();

        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let mut view_filter = filter(component_id);
        view_filter.changed_ids.push(component_id);
        let cache = cache_for(
            &world,
            view_filter,
            HashSet::from([component_id]),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        // SAFETY: the World stays live and structurally unchanged through all
        // lease/slice operations.
        let runtime = unsafe { runtime_for_world(&mut world, cache, last_run, this_run) };
        let lease = Arc::new(runtime.gather_batches().unwrap());
        let slices = lease.contiguous_slices().unwrap();

        assert_eq!(slices.len(), 2);
        assert_eq!(
            slices.iter().map(BatchSlice::len).collect::<Vec<_>>(),
            vec![1, 1]
        );
        assert_eq!(slices[0].entities().unwrap(), vec![first]);
        assert_eq!(slices[1].entities().unwrap(), vec![third]);
        assert!(matches!(
            slices[0].row(1),
            Err(ViewRuntimeError::BatchRowOutOfBounds { index: 1, len: 1 })
        ));

        let expression = RustExpr::Field {
            component_id,
            offset: 0,
            field_type: FieldType::U32,
        };
        let read = runtime.prepare_read_program(&expression).unwrap();
        assert_eq!(slices[0].evaluate(&read, false).unwrap(), vec![11.0]);
        assert_eq!(slices[1].evaluate(&read, false).unwrap(), vec![31.0]);

        let destination = FieldId {
            component_id,
            offset: 0,
            field_type: FieldType::U32,
        };
        let assignment = runtime
            .prepare_assignment_program(destination, &RustExpr::Const(99.0))
            .unwrap();
        slices[0].execute_assignment(&assignment, false).unwrap();

        runtime.validity().set_invalid();
        drop(slices);
        drop(lease);
        drop(runtime);
        assert_eq!(world.entity(first).get::<RuntimeDense>().unwrap().0, 99);
        assert_eq!(world.entity(third).get::<RuntimeDense>().unwrap().0, 31);
    }

    #[test]
    fn leased_reductions_cover_all_operations_and_complete_data_filter() {
        let mut world = World::new();
        world.spawn(RuntimeDense(10));
        world.spawn((RuntimeDense(20), RuntimeOther(2)));
        world.spawn((RuntimeDense(30), RuntimeOther(3)));
        let dense_id = world.components().component_id::<RuntimeDense>().unwrap();
        let other_id = world.components().component_id::<RuntimeOther>().unwrap();
        let view_filter = ViewFilter {
            component_ids: HashSet::from([dense_id, other_id]),
            with_ids: Vec::new(),
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        };
        let cache = cache_for(
            &world,
            view_filter,
            HashSet::new(),
            HashMap::from([
                (dense_id, HashSet::from([(0, FieldType::U32)])),
                (other_id, HashSet::from([(0, FieldType::U32)])),
            ]),
        );
        // SAFETY: the World remains live and structurally unchanged through
        // every reduction and lease operation below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
        let field_expression = RustExpr::Field {
            component_id: dense_id,
            offset: 0,
            field_type: FieldType::U32,
        };
        let field_program = runtime.prepare_read_program(&field_expression).unwrap();
        let lease = runtime.gather_batches().unwrap();

        assert_eq!(
            lease
                .reduce(&field_program, ViewReduction::Sum, true)
                .unwrap(),
            ViewReductionOutput {
                value: 50.0,
                count: 2,
            }
        );
        assert_eq!(
            lease
                .reduce(&field_program, ViewReduction::Min, false)
                .unwrap(),
            ViewReductionOutput {
                value: 20.0,
                count: 2,
            }
        );
        assert_eq!(
            lease
                .reduce(&field_program, ViewReduction::Max, true)
                .unwrap(),
            ViewReductionOutput {
                value: 30.0,
                count: 2,
            }
        );

        let truthy = RustExpr::Gt(Box::new(field_expression), Box::new(RustExpr::Const(20.0)));
        let truthy_program = runtime.prepare_read_program(&truthy).unwrap();
        assert_eq!(
            lease
                .reduce(&truthy_program, ViewReduction::CountTruthy, true)
                .unwrap(),
            ViewReductionOutput {
                value: 1.0,
                count: 2,
            }
        );
    }

    #[test]
    fn count_truthy_counts_nonzero_values_like_numpy() {
        let mut world = World::new();
        for value in [10, 20, 30] {
            world.spawn(RuntimeDense(value));
        }
        let dense_id = world.components().component_id::<RuntimeDense>().unwrap();
        let cache = cache_for(
            &world,
            filter(dense_id),
            HashSet::new(),
            HashMap::from([(dense_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        // SAFETY: the World remains live and structurally unchanged through
        // the reduction below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
        let expression = RustExpr::Sub(
            Box::new(RustExpr::Field {
                component_id: dense_id,
                offset: 0,
                field_type: FieldType::U32,
            }),
            Box::new(RustExpr::FloatConst(20.5)),
        );
        let program = runtime.prepare_read_program(&expression).unwrap();
        let lease = runtime.gather_batches().unwrap();

        let output = lease
            .reduce(&program, ViewReduction::CountTruthy, false)
            .unwrap();
        assert_eq!(
            output,
            ViewReductionOutput {
                value: 3.0,
                count: 3
            }
        );
    }

    #[test]
    fn leased_min_and_max_propagate_nan() {
        let mut world = World::new();
        world.spawn(RuntimeDense(20));
        world.spawn(RuntimeDense(30));
        let dense_id = world.components().component_id::<RuntimeDense>().unwrap();
        let view_filter = ViewFilter {
            component_ids: HashSet::from([dense_id]),
            with_ids: Vec::new(),
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        };
        let cache = cache_for(
            &world,
            view_filter,
            HashSet::new(),
            HashMap::from([(dense_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        // SAFETY: the World remains live and structurally unchanged through
        // every reduction below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
        // 0 / (value - 20) is NaN on the first row and 0.0 on the second.
        let expression = RustExpr::Div(
            Box::new(RustExpr::Const(0.0)),
            Box::new(RustExpr::Sub(
                Box::new(RustExpr::Field {
                    component_id: dense_id,
                    offset: 0,
                    field_type: FieldType::U32,
                }),
                Box::new(RustExpr::Const(20.0)),
            )),
        );
        let program = runtime.prepare_read_program(&expression).unwrap();
        let lease = runtime.gather_batches().unwrap();

        for reduction in [ViewReduction::Min, ViewReduction::Max, ViewReduction::Sum] {
            let output = lease.reduce(&program, reduction, false).unwrap();
            assert_eq!(output.count, 2, "{reduction:?}");
            assert!(output.value.is_nan(), "{reduction:?} dropped NaN");
        }
    }

    #[test]
    fn complementary_sparse_views_mutate_disjoint_rows_in_parallel() {
        let mut world = World::new();
        let plain_first = world.spawn(RuntimeDense(1)).id();
        let selected_first = world.spawn((RuntimeDense(2), RuntimeSparse)).id();
        let selected_second = world.spawn((RuntimeDense(3), RuntimeSparse)).id();
        let plain_last = world.spawn(RuntimeDense(4)).id();
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let marker_id = world.components().component_id::<RuntimeSparse>().unwrap();
        let make_filter = |with_marker| ViewFilter {
            component_ids: HashSet::from([component_id]),
            with_ids: if with_marker {
                vec![marker_id]
            } else {
                Vec::new()
            },
            without_ids: if with_marker {
                Vec::new()
            } else {
                vec![marker_id]
            },
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        };
        let allowed = || HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]);
        let with_cache = cache_for(
            &world,
            make_filter(true),
            HashSet::from([component_id]),
            allowed(),
        );
        let without_cache = cache_for(
            &world,
            make_filter(false),
            HashSet::from([component_id]),
            allowed(),
        );
        let world_cell = world.as_unsafe_world_cell();

        std::thread::scope(|scope| {
            let run = |cache: Arc<CachedViewCore>, value: u32| {
                // SAFETY: both runtimes reference this live World, and their
                // complementary With/Without filters select disjoint table rows.
                // No structural mutation occurs while either runtime is live.
                let runtime = unsafe {
                    ViewRuntimeCore::new(
                        cache,
                        world_cell,
                        ValidityFlag::new_write(),
                        Tick::new(0),
                        Tick::new(1),
                    )
                }
                .map(Arc::new)
                .unwrap();
                let lease = runtime.gather_batches().unwrap();
                lease
                    .with_batches(|batches| {
                        for batch in batches {
                            let base = batch.component_bases[&component_id];
                            for local_row in 0..batch.entity_count {
                                let table_row = batch.start_row + local_row;
                                // SAFETY: the two scoped threads operate on
                                // complementary exact row ranges, the pointer is
                                // the RuntimeDense column, and the World stays live.
                                unsafe {
                                    (*base.cast::<RuntimeDense>().add(table_row)).0 = value;
                                }
                            }
                        }
                    })
                    .unwrap();
            };

            let with_handle = scope.spawn({
                let cache = Arc::clone(&with_cache);
                move || run(cache, 100)
            });
            let without_handle = scope.spawn({
                let cache = Arc::clone(&without_cache);
                move || run(cache, 200)
            });
            with_handle.join().unwrap();
            without_handle.join().unwrap();
        });

        assert_eq!(world.get::<RuntimeDense>(plain_first).unwrap().0, 200);
        assert_eq!(world.get::<RuntimeDense>(selected_first).unwrap().0, 100);
        assert_eq!(world.get::<RuntimeDense>(selected_second).unwrap().0, 100);
        assert_eq!(world.get::<RuntimeDense>(plain_last).unwrap().0, 200);
    }

    #[test]
    fn leased_assignment_writes_and_marks_only_sparse_selected_rows() {
        let mut world = World::new();
        let plain_first = world.spawn(RuntimeDense(1)).id();
        let selected_first = world.spawn((RuntimeDense(2), RuntimeSparse)).id();
        let selected_second = world.spawn((RuntimeDense(3), RuntimeSparse)).id();
        let plain_last = world.spawn(RuntimeDense(4)).id();
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let marker_id = world.components().component_id::<RuntimeSparse>().unwrap();
        let view_filter = ViewFilter {
            component_ids: HashSet::from([component_id]),
            with_ids: vec![marker_id],
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        };
        let cache = cache_for(
            &world,
            view_filter,
            HashSet::from([component_id]),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        let this_run = Tick::new(77);
        // SAFETY: the World remains live and structurally unchanged until the
        // runtime and lease are invalidated and dropped below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), this_run) };
        let destination = field(component_id, 0, FieldType::U32);
        let program = runtime
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();
        let lease = runtime.gather_batches().unwrap();

        lease.execute_assignment(&program, true).unwrap();
        runtime.validity().set_invalid();
        drop(lease);
        drop(runtime);

        let (plain_first_value, plain_first_tick) = dense_value_and_tick(&world, plain_first);
        let (selected_first_value, selected_first_tick) =
            dense_value_and_tick(&world, selected_first);
        let (selected_second_value, selected_second_tick) =
            dense_value_and_tick(&world, selected_second);
        let (plain_last_value, plain_last_tick) = dense_value_and_tick(&world, plain_last);

        assert_eq!((plain_first_value, plain_last_value), (1, 4));
        assert_eq!((selected_first_value, selected_second_value), (42, 42));
        assert_ne!(plain_first_tick, this_run);
        assert_ne!(plain_last_tick, this_run);
        assert_eq!(selected_first_tick, this_run);
        assert_eq!(selected_second_tick, this_run);
    }

    #[test]
    fn leased_assignment_respects_tick_mask_for_writes_and_change_marks() {
        let mut world = World::new();
        let old = world.spawn(RuntimeDense(1)).id();
        world.clear_trackers();
        let last_run = world.last_change_tick();
        world.increment_change_tick();
        let added = world.spawn(RuntimeDense(2)).id();
        let this_run = world.change_tick();
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let mut view_filter = filter(component_id);
        view_filter.added_ids.push(component_id);
        let cache = cache_for(
            &world,
            view_filter,
            HashSet::from([component_id]),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        // SAFETY: the World remains live and structurally unchanged until the
        // runtime and lease are invalidated and dropped below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, last_run, this_run) };
        let destination = field(component_id, 0, FieldType::U32);
        let program = runtime
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();
        let lease = runtime.gather_batches().unwrap();

        lease.execute_assignment(&program, false).unwrap();
        runtime.validity().set_invalid();
        drop(lease);
        drop(runtime);

        assert_eq!(dense_value_and_tick(&world, old).0, 1);
        assert_eq!(dense_value_and_tick(&world, added), (42, this_run));
    }

    #[test]
    fn rejected_program_intent_writes_and_marks_nothing() {
        let mut world = World::new();
        let entity = world.spawn(RuntimeDense(5)).id();
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let cache = cache_for(
            &world,
            filter(component_id),
            HashSet::from([component_id]),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        let this_run = Tick::new(88);
        // SAFETY: the World remains live and structurally unchanged until the
        // runtime and lease are invalidated and dropped below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), this_run) };
        let read = runtime
            .validate_program(
                read_program(field(component_id, 0, FieldType::U32)),
                ProgramIntent::ReadOnly,
            )
            .unwrap();
        let destination = field(component_id, 0, FieldType::U32);
        let assignment = runtime
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();
        let lease = runtime.gather_batches().unwrap();

        assert!(matches!(
            lease.execute_assignment(&read, false),
            Err(ViewRuntimeError::ProgramIntentMismatch {
                expected: "assignment",
                actual: "read-only",
            })
        ));
        assert!(matches!(
            lease.reduce(&assignment, ViewReduction::Sum, false),
            Err(ViewRuntimeError::ProgramIntentMismatch {
                expected: "read-only",
                actual: "assignment",
            })
        ));
        runtime.validity().set_invalid();
        drop(lease);
        drop(runtime);

        let (value, tick) = dense_value_and_tick(&world, entity);
        assert_eq!(value, 5);
        assert_ne!(tick, this_run);
    }

    #[test]
    fn cross_runtime_program_and_stale_lease_write_and_mark_nothing() {
        let mut world = World::new();
        let entity = world.spawn(RuntimeDense(5)).id();
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let cache = cache_for(
            &world,
            filter(component_id),
            HashSet::from([component_id]),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        let this_run = Tick::new(89);
        // SAFETY: the World remains live and structurally unchanged until the
        // runtime and lease are invalidated and dropped below.
        let owner = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), this_run) };
        let lease = owner.gather_batches().unwrap();
        let foreign = runtime(component_id, true);
        let destination = field(component_id, 0, FieldType::F32);
        let foreign_program = foreign
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();

        assert!(matches!(
            lease.execute_assignment(&foreign_program, false),
            Err(ViewRuntimeError::ProgramFromDifferentRuntime)
        ));
        owner.validity().set_invalid();
        assert!(matches!(
            lease.execute_assignment(&foreign_program, false),
            Err(ViewRuntimeError::ProgramFromDifferentRuntime)
        ));

        drop(lease);
        drop(owner);
        let (value, tick) = dense_value_and_tick(&world, entity);
        assert_eq!(value, 5);
        assert_ne!(tick, this_run);
    }

    #[test]
    fn stale_lease_rejects_own_assignment_before_change_tick_resolution() {
        let mut world = World::new();
        let entity = world.spawn(RuntimeDense(5)).id();
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let cache = cache_for(
            &world,
            filter(component_id),
            HashSet::from([component_id]),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        let this_run = Tick::new(90);
        // SAFETY: the World remains live and structurally unchanged until the
        // runtime and lease are invalidated and dropped below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), this_run) };
        let destination = field(component_id, 0, FieldType::U32);
        let program = runtime
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();
        let lease = runtime.gather_batches().unwrap();
        runtime.validity().set_invalid();

        assert!(matches!(
            lease.execute_assignment(&program, false),
            Err(ViewRuntimeError::Storage(StorageError::InvalidAccess))
        ));

        drop(lease);
        drop(runtime);
        let (value, tick) = dense_value_and_tick(&world, entity);
        assert_eq!(value, 5);
        assert_ne!(tick, this_run);
    }

    #[test]
    fn partial_write_panic_marks_prevalidated_rows_before_releasing_fence() {
        let mut world = World::new();
        let first = world.spawn(RuntimeDense(1)).id();
        let second = world.spawn(RuntimeDense(2)).id();
        let component_id = world.components().component_id::<RuntimeDense>().unwrap();
        let cache = cache_for(
            &world,
            filter(component_id),
            HashSet::from([component_id]),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::U32)]))]),
        );
        let this_run = Tick::new(99);
        // SAFETY: the World remains live and structurally unchanged until the
        // runtime and lease are invalidated and dropped below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), this_run) };
        let destination = field(component_id, 0, FieldType::U32);
        let program = runtime
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();
        let lease = runtime.gather_batches().unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = lease.execute_assignment_with(&program, |batches, _bytecode, _strides| {
                let batch = &batches[0];
                let base = batch.component_bases[&component_id];
                // SAFETY: this is the first exact selected row of the live
                // RuntimeDense column under the assignment operation fence.
                unsafe {
                    (*base.cast::<RuntimeDense>().add(batch.start_row)).0 = 123;
                }
                panic!("injected panic after first raw View write");
            });
        }));
        assert!(result.is_err());
        assert!(runtime.enter_operation().is_ok());

        runtime.validity().set_invalid();
        drop(lease);
        drop(runtime);

        assert_eq!(dense_value_and_tick(&world, first), (123, this_run));
        assert_eq!(dense_value_and_tick(&world, second), (2, this_run));
    }

    #[derive(Component)]
    #[repr(C)]
    struct BoolProbe {
        flag: bool,
        value: f32,
    }

    #[test]
    fn bool_field_arithmetic_is_rejected_at_validation() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, false);
        let result = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::PushField(0), Op::PushConst(0), Op::Add],
                constants: vec![1.0],
                integers: Vec::new(),
                field_map: vec![field(component_id, 4, FieldType::Bool)],
            }),
            ProgramIntent::ReadOnly,
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::StackTypeMismatch {
                instruction: "Add",
                expected: "float",
                actual: "bool",
                ..
            })
        ));
    }

    #[test]
    fn bool_field_store_to_numeric_destination_is_rejected() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 0, FieldType::F32);
        let runtime = runtime(component_id, true);
        let result = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::PushField(1), Op::StoreField(0)],
                constants: Vec::new(),
                integers: Vec::new(),
                field_map: vec![destination, field(component_id, 4, FieldType::Bool)],
            }),
            ProgramIntent::Assignment { destination },
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::StackTypeMismatch {
                instruction: "StoreField",
                expected: "float",
                actual: "bool",
                ..
            })
        ));
    }

    #[test]
    fn bool_destination_accepts_numeric_and_boolean_values() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 4, FieldType::Bool);
        let runtime = runtime(component_id, true);

        let numeric = runtime.validate_program(
            assignment_program(destination),
            ProgramIntent::Assignment { destination },
        );
        assert!(numeric.is_ok());

        let boolean = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![
                    Op::PushConst(0),
                    Op::PushConst(0),
                    Op::Eq,
                    Op::StoreField(0),
                ],
                constants: vec![1.0],
                integers: Vec::new(),
                field_map: vec![destination],
            }),
            ProgramIntent::Assignment { destination },
        );
        assert!(boolean.is_ok());
    }

    #[test]
    fn bool_field_read_program_validates() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, false);
        let result = runtime.validate_program(
            read_program(field(component_id, 4, FieldType::Bool)),
            ProgramIntent::ReadOnly,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn bool_field_where_condition_validates_and_executes() {
        let mut world = World::new();
        let on = world
            .spawn(BoolProbe {
                flag: true,
                value: 0.0,
            })
            .id();
        let off = world
            .spawn(BoolProbe {
                flag: false,
                value: 0.0,
            })
            .id();
        let component_id = world.components().component_id::<BoolProbe>().unwrap();
        let cache = cache_for(
            &world,
            filter(component_id),
            HashSet::from([component_id]),
            HashMap::from([(
                component_id,
                HashSet::from([(0, FieldType::Bool), (4, FieldType::F32)]),
            )]),
        );
        // SAFETY: the World remains live and structurally unchanged until the
        // runtime and lease are invalidated and dropped below.
        let runtime = unsafe { runtime_for_world(&mut world, cache, Tick::new(0), Tick::new(1)) };
        let destination = field(component_id, 4, FieldType::F32);
        let expression = RustExpr::Where(
            Box::new(RustExpr::Field {
                component_id,
                offset: 0,
                field_type: FieldType::Bool,
            }),
            Box::new(RustExpr::Const(2.0)),
            Box::new(RustExpr::Const(3.0)),
        );
        let program = runtime
            .prepare_assignment_program(destination, &expression)
            .unwrap();
        let lease = runtime.gather_batches().unwrap();

        lease.execute_assignment(&program, false).unwrap();
        runtime.validity().set_invalid();
        drop(lease);
        drop(runtime);

        assert_eq!(world.entity(on).get::<BoolProbe>().unwrap().value, 2.0);
        assert_eq!(world.entity(off).get::<BoolProbe>().unwrap().value, 3.0);
    }

    #[test]
    fn mixed_integer_width_arithmetic_is_rejected_before_execution() {
        let component_id = ComponentId::new(1);
        let bytecode = CompiledBytecode {
            bytecode: vec![Op::PushField(0), Op::PushField(1), Op::Add],
            constants: vec![],
            integers: Vec::new(),
            field_map: vec![
                field(component_id, 0, FieldType::I64),
                field(component_id, 8, FieldType::U32),
            ],
        };
        assert!(matches!(
            validate_integer_arithmetic_types(&bytecode),
            Err(ViewRuntimeError::MixedIntegerArithmetic { .. })
        ));
    }

    #[test]
    fn integer_field_and_constant_keep_the_field_width() {
        let component_id = ComponentId::new(1);
        let bytecode = CompiledBytecode {
            bytecode: vec![Op::PushField(0), Op::PushConst(0), Op::Add],
            constants: vec![1.0],
            integers: Vec::new(),
            field_map: vec![field(component_id, 0, FieldType::I64)],
        };
        assert!(validate_integer_arithmetic_types(&bytecode).is_ok());
    }

    fn integer_assignment(operations: &[Op], constants: Vec<f64>) -> CompiledBytecode {
        let destination = field(ComponentId::new(1), 0, FieldType::I64);
        let mut bytecode = operations.to_vec();
        bytecode.push(Op::StoreField(0));
        CompiledBytecode {
            bytecode,
            constants,
            integers: Vec::new(),
            field_map: vec![destination],
        }
    }

    #[test]
    fn integral_float_literal_promotes_integer_field_to_float() {
        let float_literal = integer_assignment(
            &[Op::PushField(0), Op::PushFloatConst(0), Op::FloorDiv],
            vec![2.0],
        );
        assert!(matches!(
            validate_integer_arithmetic_types(&float_literal),
            Err(ViewRuntimeError::ImplicitFloatToInteger {
                destination: FieldType::I64
            })
        ));
        let int_literal = integer_assignment(
            &[Op::PushField(0), Op::PushConst(0), Op::FloorDiv],
            vec![2.0],
        );
        assert!(validate_integer_arithmetic_types(&int_literal).is_ok());
        let float_store = integer_assignment(&[Op::PushFloatConst(0)], vec![2.0]);
        assert!(validate_integer_arithmetic_types(&float_store).is_err());
    }

    #[test]
    fn integer_power_keeps_integer_type_and_rejects_negative_literal_exponent() {
        let square = integer_assignment(&[Op::PushField(0), Op::PushConst(0), Op::Pow], vec![2.0]);
        assert!(validate_integer_arithmetic_types(&square).is_ok());
        let field_exponent =
            integer_assignment(&[Op::PushConst(0), Op::PushField(0), Op::Pow], vec![2.0]);
        assert!(validate_integer_arithmetic_types(&field_exponent).is_ok());

        for operations in [
            &[Op::PushField(0), Op::PushConst(0), Op::Pow][..],
            &[Op::PushField(0), Op::PushConst(1), Op::Neg, Op::Pow][..],
            &[Op::PushConst(1), Op::PushConst(0), Op::Pow][..],
        ] {
            let program = integer_assignment(operations, vec![-1.0, 1.0]);
            assert!(matches!(
                validate_integer_arithmetic_types(&program),
                Err(ViewRuntimeError::NegativeIntegerPower)
            ));
        }
    }

    #[test]
    fn float_power_is_float_and_accepts_negative_exponents() {
        let float_exponent = integer_assignment(
            &[Op::PushField(0), Op::PushFloatConst(0), Op::Pow],
            vec![-1.0],
        );
        assert!(matches!(
            validate_integer_arithmetic_types(&float_exponent),
            Err(ViewRuntimeError::ImplicitFloatToInteger { .. })
        ));
        let component_id = ComponentId::new(1);
        let float_base = CompiledBytecode {
            bytecode: vec![
                Op::PushField(0),
                Op::PushConst(0),
                Op::Pow,
                Op::StoreField(0),
            ],
            constants: vec![-1.0],
            integers: Vec::new(),
            field_map: vec![field(component_id, 0, FieldType::F64)],
        };
        assert!(validate_integer_arithmetic_types(&float_base).is_ok());
    }

    type LiteralShape<'a> = (FieldType, &'a [Option<Op>], &'a [i128]);
    type RejectedLiteral<'a> = (FieldType, &'a [Option<Op>], &'a [i128], &'a str);

    fn literal_program(
        field_type: FieldType,
        shape: &[Option<Op>],
        literals: &[i128],
    ) -> CompiledBytecode {
        let mut compiler = Compiler::new();
        let field_index = compiler.add_field(field(ComponentId::new(1), 0, field_type));
        let mut literals = literals.iter();
        for op in shape {
            let op = match op {
                Some(op) => *op,
                None => compiler.int_literal(*literals.next().expect("one literal per slot")),
            };
            compiler.emit(op);
        }
        compiler.emit(Op::StoreField(field_index));
        compiler.finalize()
    }

    fn out_of_bounds(literal: &str, field_type: FieldType) -> ViewRuntimeError {
        ViewRuntimeError::IntegerLiteralOutOfBounds(IntegerLiteralOutOfBounds {
            literal: literal.to_owned(),
            field_type,
        })
    }

    #[test]
    fn int_literal_outside_its_integer_lane_is_rejected() {
        let add = [Some(Op::PushField(0)), None, Some(Op::Add)];
        let reversed = [None, Some(Op::PushField(0)), Some(Op::Sub)];
        let power = [Some(Op::PushField(0)), None, Some(Op::Pow)];
        let store = [None];
        let clip = [Some(Op::PushField(0)), None, None, Some(Op::Clamp)];
        let rejected: [RejectedLiteral; 11] = [
            (FieldType::U8, &add, &[300], "300"),
            (FieldType::U8, &reversed, &[-1], "-1"),
            (FieldType::U8, &store, &[256], "256"),
            (FieldType::U32, &power, &[-1], "-1"),
            (FieldType::I32, &add, &[1 << 31], "2147483648"),
            (FieldType::I32, &store, &[-(1 << 31) - 1], "-2147483649"),
            (FieldType::I64, &add, &[1 << 63], "9223372036854775808"),
            (
                FieldType::I64,
                &reversed,
                &[-(1 << 63) - 1],
                "-9223372036854775809",
            ),
            (FieldType::U64, &add, &[1 << 64], "18446744073709551616"),
            (FieldType::U32, &clip, &[1 << 40, 1 << 41], "1099511627776"),
            (FieldType::U32, &clip, &[-5, -1], "-1"),
        ];
        for (field_type, shape, literals, message) in rejected {
            let program = literal_program(field_type, shape, literals);
            let expected = out_of_bounds(message, field_type).to_string();
            assert_eq!(
                validate_integer_arithmetic_types(&program).map_err(|error| error.to_string()),
                Err(expected),
                "{field_type:?} {literals:?}"
            );
        }
        let accepted: [LiteralShape; 7] = [
            (FieldType::U8, &add, &[255]),
            (FieldType::I32, &add, &[-(1 << 31)]),
            (FieldType::I64, &add, &[i64::MAX.into()]),
            (FieldType::I64, &reversed, &[i64::MIN.into()]),
            (FieldType::U64, &store, &[u64::MAX.into()]),
            (FieldType::U32, &clip, &[-1, 1 << 40]),
            (FieldType::F32, &add, &[1 << 100]),
        ];
        for (field_type, shape, literals) in accepted {
            let program = literal_program(field_type, shape, literals);
            assert!(
                validate_integer_arithmetic_types(&program).is_ok(),
                "{field_type:?} {literals:?}"
            );
        }
        assert_eq!(
            out_of_bounds("18446744073709551616", FieldType::U64).to_string(),
            "Python integer 18446744073709551616 out of bounds for uint64"
        );
    }

    #[test]
    fn buffer_stores_cast_floats_but_check_literal_bounds() {
        let float_store = literal_program(
            FieldType::U8,
            &[
                Some(Op::PushField(0)),
                Some(Op::PushFloatConst(0)),
                Some(Op::Mul),
            ],
            &[],
        );
        let mut float_store = float_store;
        float_store.constants.push(2.0);
        assert!(check_integer_types(&float_store, StoreCasting::Unsafe).is_ok());
        assert!(check_integer_types(&float_store, StoreCasting::SameKind).is_err());
        let literal = literal_program(
            FieldType::U8,
            &[Some(Op::PushField(0)), None, Some(Op::Add)],
            &[300],
        );
        assert!(matches!(
            check_integer_types(&literal, StoreCasting::Unsafe),
            Err(ViewRuntimeError::IntegerLiteralOutOfBounds(_))
        ));
    }

    #[test]
    fn signed_integer_store_into_unsigned_field_follows_same_kind() {
        let store = |source: FieldType, destination: FieldType| {
            let component_id = ComponentId::new(1);
            CompiledBytecode {
                bytecode: vec![Op::PushField(0), Op::StoreField(1)],
                constants: Vec::new(),
                integers: Vec::new(),
                field_map: vec![
                    field(component_id, 0, source),
                    field(component_id, 8, destination),
                ],
            }
        };
        for (source, destination) in [
            (FieldType::I64, FieldType::U64),
            (FieldType::I32, FieldType::U8),
            (FieldType::I64, FieldType::U32),
        ] {
            let program = store(source, destination);
            assert!(matches!(
                validate_integer_arithmetic_types(&program),
                Err(ViewRuntimeError::ImplicitSignedToUnsigned { destination: d }) if d == destination
            ));
            assert!(check_integer_types(&program, StoreCasting::Unsafe).is_ok());
        }
        for (source, destination) in [
            (FieldType::I64, FieldType::I32),
            (FieldType::U64, FieldType::I64),
            (FieldType::U32, FieldType::U8),
            (FieldType::U8, FieldType::I32),
        ] {
            assert!(validate_integer_arithmetic_types(&store(source, destination)).is_ok());
        }
    }

    #[test]
    fn integer_power_rejects_mixed_integer_widths() {
        let component_id = ComponentId::new(1);
        let bytecode = CompiledBytecode {
            bytecode: vec![Op::PushField(0), Op::PushField(1), Op::Pow],
            constants: vec![],
            integers: Vec::new(),
            field_map: vec![
                field(component_id, 0, FieldType::I64),
                field(component_id, 8, FieldType::I32),
            ],
        };
        assert!(matches!(
            validate_integer_arithmetic_types(&bytecode),
            Err(ViewRuntimeError::MixedIntegerArithmetic {
                instruction: "Pow",
                ..
            })
        ));
    }
}
