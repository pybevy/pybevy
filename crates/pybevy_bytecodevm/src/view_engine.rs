//! Shared batch execution engine for the View API.
//!
//! Handles archetype iteration, field pointer resolution, and bytecode
//! execution across entity batches. Python-agnostic — works only with
//! bevy_ecs types and the bytecodevm's CompiledBytecode/VM.

use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
};

use bevy_ecs::{
    change_detection::Tick,
    component::ComponentId,
    prelude::QueryBuilder,
    storage::{Table, TableId},
    world::{FilteredEntityMut, World},
};
use smallvec::SmallVec;

use crate::{
    bytecode::{CompiledBytecode, Compiler, FieldId, FieldType, Op, VM},
    expr::RustExpr,
};

/// Maximum field pointers to stack-allocate before heap fallback.
type FieldPtrVec = SmallVec<[*mut u8; 8]>;

/// Send+Sync wrapper for raw pointers in parallel execution.
///
/// # Safety
/// Pointers are valid for the duration of batch execution and accessed
/// using proper rayon semantics (no aliasing writes).
#[derive(Clone, Copy)]
struct SendPtr(*mut u8);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

/// A batch of entities from one archetype table with raw column pointers.
pub struct TableBatch {
    /// The table this batch was gathered from.
    pub table_id: TableId,
    /// Base pointer for each component's column data in this table.
    pub component_bases: HashMap<ComponentId, *mut u8>,
    /// Number of entities in this table.
    pub entity_count: usize,
    /// Per-entity tick mask. `None` means all entities pass.
    /// `Some(vec)` where `vec[i]` is true if entity i passes all tick filters.
    pub tick_mask: Option<Vec<bool>>,
}

/// Filter criteria for archetype matching.
///
/// All IDs must be pre-resolved before calling view engine functions.
pub struct ViewFilter {
    /// Component IDs that must be present (the query components).
    pub component_ids: HashSet<ComponentId>,
    /// With-filter component IDs (must be present on archetype).
    pub with_ids: Vec<ComponentId>,
    /// Without-filter component IDs (must NOT be present on archetype).
    pub without_ids: Vec<ComponentId>,
    /// Changed-filter component IDs (must be present; per-entity tick check).
    pub changed_ids: Vec<ComponentId>,
    /// Added-filter component IDs (must be present; per-entity tick check).
    pub added_ids: Vec<ComponentId>,
}

/// Error type for view engine operations.
#[derive(Debug)]
pub enum ViewEngineError {
    /// A required component was not found in the World's component registry.
    ComponentNotFound(ComponentId),
}

impl std::fmt::Display for ViewEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewEngineError::ComponentNotFound(id) => {
                write!(f, "Component {:?} not found in World", id)
            }
        }
    }
}

impl std::error::Error for ViewEngineError {}

/// Resolve component strides (layout sizes) from the World's component metadata.
pub fn resolve_component_strides(
    world: &World,
    component_ids: &HashSet<ComponentId>,
) -> Result<HashMap<ComponentId, usize>, ViewEngineError> {
    let components = world.components();
    let mut strides = HashMap::with_capacity(component_ids.len());
    for &component_id in component_ids {
        let info = components
            .get_info(component_id)
            .ok_or(ViewEngineError::ComponentNotFound(component_id))?;
        strides.insert(component_id, info.layout().size());
    }
    Ok(strides)
}

/// Build a per-entity tick mask for a table.
///
/// Returns `None` if there are no tick filters (all entities pass).
fn build_tick_mask_for_table(
    table: &Table,
    entity_count: usize,
    changed_ids: &[ComponentId],
    added_ids: &[ComponentId],
    last_run: Tick,
    this_run: Tick,
) -> Option<Vec<bool>> {
    if changed_ids.is_empty() && added_ids.is_empty() {
        return None;
    }

    let mut mask = vec![true; entity_count];

    for &id in changed_ids {
        if let Some(column) = table.get_column(id) {
            let changed_ticks = unsafe { column.get_changed_ticks_slice(entity_count) };
            for i in 0..entity_count {
                if mask[i] {
                    let tick = unsafe { *changed_ticks[i].get() };
                    if !tick.is_newer_than(last_run, this_run) {
                        mask[i] = false;
                    }
                }
            }
        }
    }

    for &id in added_ids {
        if let Some(column) = table.get_column(id) {
            let added_ticks = unsafe { column.get_added_ticks_slice(entity_count) };
            for i in 0..entity_count {
                if mask[i] {
                    let tick = unsafe { *added_ticks[i].get() };
                    if !tick.is_newer_than(last_run, this_run) {
                        mask[i] = false;
                    }
                }
            }
        }
    }

    Some(mask)
}

/// Gather all archetype table batches matching the filter criteria.
///
/// Returns mutable raw pointers into component columns, valid for both
/// reads and writes. The caller must not modify World storage (spawn,
/// despawn, add/remove components) while the returned pointers are live.
/// Concurrent writes to the same field from multiple threads are the
/// caller's responsibility to prevent (the batch execution functions
/// handle this via disjoint entity ranges).
pub fn gather_table_batches(
    world: &mut World,
    filter: &ViewFilter,
    last_run: Tick,
    this_run: Tick,
) -> Vec<TableBatch> {
    let has_tick_filters = !filter.changed_ids.is_empty() || !filter.added_ids.is_empty();

    let archetypes = world.archetypes();
    let storages = world.storages();
    let tables = &storages.tables;

    let mut batches = Vec::new();
    for archetype in archetypes.iter() {
        if !filter
            .component_ids
            .iter()
            .all(|id| archetype.contains(*id))
        {
            continue;
        }
        if !filter.with_ids.iter().all(|id| archetype.contains(*id)) {
            continue;
        }
        if filter.without_ids.iter().any(|id| archetype.contains(*id)) {
            continue;
        }
        if !filter.changed_ids.iter().all(|id| archetype.contains(*id)) {
            continue;
        }
        if !filter.added_ids.iter().all(|id| archetype.contains(*id)) {
            continue;
        }

        let table_id = archetype.table_id();
        if let Some(table) = tables.get(table_id) {
            let entity_count = table.entity_count() as usize;
            if entity_count > 0 {
                let tick_mask = if has_tick_filters {
                    build_tick_mask_for_table(
                        table,
                        entity_count,
                        &filter.changed_ids,
                        &filter.added_ids,
                        last_run,
                        this_run,
                    )
                } else {
                    None
                };

                // Skip entire table if tick mask filters out all entities
                if let Some(ref mask) = tick_mask {
                    if !mask.iter().any(|&v| v) {
                        continue;
                    }
                }

                let mut component_bases: HashMap<ComponentId, *mut u8> = HashMap::new();
                let mut all_found = true;
                for &component_id in &filter.component_ids {
                    if let Some(column) = table.get_column(component_id) {
                        let ptr = unsafe {
                            let data_slice = column.get_data_slice::<u8>(entity_count);
                            data_slice.as_ptr() as *mut u8
                        };
                        component_bases.insert(component_id, ptr);
                    } else {
                        all_found = false;
                        break;
                    }
                }

                if all_found {
                    batches.push(TableBatch {
                        table_id,
                        component_bases,
                        entity_count,
                        tick_mask,
                    });
                }
            }
        }
    }
    batches
}

/// Build field pointers for a single entity from batch base pointers.
///
/// # Safety
///
/// All base pointers in `component_bases` must be valid for the entity at `entity_idx`.
pub unsafe fn build_entity_field_ptrs(
    bytecode: &CompiledBytecode,
    component_bases: &HashMap<ComponentId, *mut u8>,
    field_strides: &[usize],
    entity_idx: usize,
) -> FieldPtrVec {
    let mut ptrs: FieldPtrVec = SmallVec::with_capacity(bytecode.field_map.len());
    for (i, field_id) in bytecode.field_map.iter().enumerate() {
        let base = component_bases[&field_id.component_id];
        let stride = field_strides[i];
        ptrs.push(unsafe { base.add(field_id.offset).add(entity_idx * stride) });
    }
    ptrs
}

/// Execute bytecode on a single entity's component data (write mode).
///
/// # Safety
///
/// `data_ptr` must be valid and point to the component's data with
/// correct layout for all field offsets in the bytecode.
pub unsafe fn execute_on_ptr(data_ptr: *mut u8, bytecode: &CompiledBytecode) {
    // PERF: consider PooledVM::acquire() to reuse stack allocation
    let mut vm = VM::new();
    let mut field_ptrs: FieldPtrVec = SmallVec::with_capacity(bytecode.field_map.len());
    for field_id in &bytecode.field_map {
        field_ptrs.push(unsafe { data_ptr.add(field_id.offset) });
    }
    let entity_seed = data_ptr as usize;
    unsafe { vm.execute(bytecode, field_ptrs.as_slice(), entity_seed) };
}

/// Evaluate bytecode on a single entity's component data (read-only, returns scalar).
///
/// # Safety
///
/// `data_ptr` must be valid and point to the component's data with
/// correct layout for all field offsets in the bytecode.
pub unsafe fn evaluate_on_ptr(data_ptr: *const u8, bytecode: &CompiledBytecode) -> f64 {
    // PERF: consider PooledVM::acquire() to reuse stack allocation
    let mut vm = VM::new();
    let mut field_ptrs: FieldPtrVec = SmallVec::with_capacity(bytecode.field_map.len());
    for field_id in &bytecode.field_map {
        field_ptrs.push(unsafe { data_ptr.add(field_id.offset) as *mut u8 });
    }
    let entity_seed = data_ptr as usize;
    unsafe { vm.execute_and_reduce(bytecode, field_ptrs.as_slice(), entity_seed) }
}

/// Execute bytecode assignment across all entities in all batches (fast path).
///
/// When `parallel` is true (and the `parallel` feature is enabled), uses
/// rayon chunked execution for multi-threaded processing.
///
/// # Safety
///
/// All pointers in `batches` must be valid for the duration of this call.
/// No other code may mutate the same memory concurrently.
pub unsafe fn execute_batch_assignment(
    batches: &[TableBatch],
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
    parallel: bool,
) {
    const CHUNK_SIZE: usize = 32768;

    struct ChunkInfo {
        field_bases: Vec<SendPtr>,
        field_strides: Vec<usize>,
        count: usize,
    }

    let chunks: Vec<ChunkInfo> = batches
        .iter()
        .flat_map(|batch| {
            let field_strides: Vec<usize> = bytecode
                .field_map
                .iter()
                .map(|field_id| component_strides[&field_id.component_id])
                .collect();

            (0..batch.entity_count)
                .step_by(CHUNK_SIZE)
                .map(move |start| {
                    let chunk_count = (batch.entity_count - start).min(CHUNK_SIZE);
                    let field_bases: Vec<SendPtr> = bytecode
                        .field_map
                        .iter()
                        .enumerate()
                        .map(|(i, field_id)| {
                            let component_base = batch.component_bases[&field_id.component_id];
                            let stride = field_strides[i];
                            SendPtr(unsafe {
                                component_base.add(field_id.offset).add(start * stride)
                            })
                        })
                        .collect();
                    ChunkInfo {
                        field_bases,
                        field_strides: field_strides.clone(),
                        count: chunk_count,
                    }
                })
        })
        .collect();

    let process_chunk = |chunk: &ChunkInfo| {
        // PERF: consider PooledVM::acquire() to reuse stack allocation
        let mut vm = VM::new();
        let bases: Vec<*mut u8> = chunk.field_bases.iter().map(|p| p.0).collect();
        unsafe {
            vm.execute_batch_multi(bytecode, &bases, &chunk.field_strides, chunk.count);
        }
    };

    #[cfg(feature = "parallel")]
    if parallel {
        use rayon::prelude::*;
        chunks.par_iter().for_each(process_chunk);
        return;
    }

    let _ = parallel;
    for chunk in &chunks {
        process_chunk(chunk);
    }
}

/// Execute bytecode assignment on tick-filtered entities only.
///
/// Processes each passing entity individually (cannot use batch VM because
/// entities may be non-contiguous after tick filtering).
///
/// # Safety
///
/// All pointers in `batches` must be valid. Tick masks must accurately
/// reflect which entities should be processed.
pub unsafe fn execute_filtered_assignment(
    batches: &[TableBatch],
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
    parallel: bool,
) {
    let field_strides: Vec<usize> = bytecode
        .field_map
        .iter()
        .map(|field_id| component_strides[&field_id.component_id])
        .collect();

    struct EntityWork {
        field_ptrs: Vec<SendPtr>,
        entity_seed: usize,
    }

    let work_items: Vec<EntityWork> = batches
        .iter()
        .flat_map(|batch| {
            let mask = batch.tick_mask.as_ref();
            let strides = &field_strides;
            (0..batch.entity_count).filter_map(move |entity_idx| {
                if let Some(mask) = mask {
                    if !mask[entity_idx] {
                        return None;
                    }
                }

                let field_ptrs: Vec<SendPtr> = bytecode
                    .field_map
                    .iter()
                    .enumerate()
                    .map(|(i, field_id)| {
                        let base = batch.component_bases[&field_id.component_id];
                        let stride = strides[i];
                        SendPtr(unsafe { base.add(field_id.offset).add(entity_idx * stride) })
                    })
                    .collect();

                Some(EntityWork {
                    field_ptrs,
                    entity_seed: entity_idx,
                })
            })
        })
        .collect();

    let process_entity = |work: &EntityWork| {
        // PERF: consider PooledVM::acquire() to reuse stack allocation
        let mut vm = VM::new();
        let ptrs: Vec<*mut u8> = work.field_ptrs.iter().map(|p| p.0).collect();
        unsafe {
            vm.execute(bytecode, &ptrs, work.entity_seed);
        }
    };

    #[cfg(feature = "parallel")]
    if parallel {
        use rayon::prelude::*;
        work_items.par_iter().for_each(process_entity);
        return;
    }

    let _ = parallel;
    for work in &work_items {
        process_entity(work);
    }
}

/// Mark destination component as changed on all matching entities.
///
/// Reuses the already-gathered batches (and their tick masks) to avoid
/// re-iterating archetypes and recomputing tick filters.
///
/// Required after raw pointer writes that bypass Bevy's `DerefMut`.
/// Without this, systems using `Changed<T>` won't see the updates.
pub fn mark_component_changed(
    world: &mut World,
    batches: &[TableBatch],
    dest_component_id: ComponentId,
) {
    let change_tick = world.change_tick();
    let tables = &world.storages().tables;

    for batch in batches {
        let Some(table) = tables.get(batch.table_id) else {
            continue;
        };
        let Some(column) = table.get_column(dest_component_id) else {
            continue;
        };

        let changed_ticks = unsafe { column.get_changed_ticks_slice(batch.entity_count) };

        if let Some(ref mask) = batch.tick_mask {
            for i in 0..batch.entity_count {
                if mask[i] {
                    unsafe {
                        *changed_ticks[i].get() = change_tick;
                    }
                }
            }
        } else {
            for tick in changed_ticks {
                unsafe {
                    *tick.get() = change_tick;
                }
            }
        }
    }
}

/// Key for bytecode cache: (component_id, field_offset, expression_hash).
type CacheKey = (ComponentId, usize, u64);

/// Frame-persistent cache for compiled bytecodes.
///
/// Store per-system (or thread-local) to avoid recompiling the same
/// expression every frame. The cache is keyed by destination field +
/// expression identity.
#[derive(Clone, Default)]
pub struct BytecodeCache {
    cache: HashMap<CacheKey, CompiledBytecode>,
}

impl BytecodeCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Hash a RustExpr for cache lookup.
    pub fn expr_hash(expr: &RustExpr) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", expr).hash(&mut hasher);
        hasher.finish()
    }

    /// Get cached bytecode or compile and cache it.
    pub fn get_or_compile(
        &mut self,
        dest_component_id: ComponentId,
        dest_offset: usize,
        dest_field_type: FieldType,
        expr: &RustExpr,
        expr_hash: u64,
    ) -> &CompiledBytecode {
        let key: CacheKey = (dest_component_id, dest_offset, expr_hash);
        self.cache.entry(key).or_insert_with(|| {
            compile_assignment(dest_component_id, dest_offset, dest_field_type, expr)
        })
    }
}

/// Compile a field assignment expression to bytecode.
pub fn compile_assignment(
    dest_component_id: ComponentId,
    dest_offset: usize,
    dest_field_type: FieldType,
    expr: &RustExpr,
) -> CompiledBytecode {
    let mut compiler = Compiler::new();
    compile_expr(expr, &mut compiler);
    let dest = FieldId {
        component_id: dest_component_id,
        offset: dest_offset,
        field_type: dest_field_type,
    };
    let dest_idx = compiler.add_field(dest);
    compiler.emit(Op::StoreField(dest_idx));
    compiler.finalize()
}

/// Compile a RustExpr to bytecode operations.
fn compile_expr(expr: &RustExpr, c: &mut Compiler) {
    match expr {
        RustExpr::Field {
            component_id,
            offset,
            field_type,
        } => {
            let fid = FieldId {
                component_id: *component_id,
                offset: *offset,
                field_type: *field_type,
            };
            let idx = c.add_field(fid);
            c.emit(Op::PushField(idx));
        }
        RustExpr::Const(v) => {
            let idx = c.add_constant(*v);
            c.emit(Op::PushConst(idx));
        }
        RustExpr::Add(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Add);
        }
        RustExpr::Sub(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Sub);
        }
        RustExpr::Mul(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Mul);
        }
        RustExpr::Div(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Div);
        }
        RustExpr::Neg(a) => {
            compile_expr(a, c);
            c.emit(Op::Neg);
        }
        RustExpr::Sin(a) => {
            compile_expr(a, c);
            c.emit(Op::Sin);
        }
        RustExpr::Cos(a) => {
            compile_expr(a, c);
            c.emit(Op::Cos);
        }
        RustExpr::Tan(a) => {
            compile_expr(a, c);
            c.emit(Op::Tan);
        }
        RustExpr::Asin(a) => {
            compile_expr(a, c);
            c.emit(Op::Asin);
        }
        RustExpr::Acos(a) => {
            compile_expr(a, c);
            c.emit(Op::Acos);
        }
        RustExpr::Atan(a) => {
            compile_expr(a, c);
            c.emit(Op::Atan);
        }
        RustExpr::Sqrt(a) => {
            compile_expr(a, c);
            c.emit(Op::Sqrt);
        }
        RustExpr::Abs(a) => {
            compile_expr(a, c);
            c.emit(Op::Abs);
        }
        RustExpr::Floor(a) => {
            compile_expr(a, c);
            c.emit(Op::Floor);
        }
        RustExpr::Ceil(a) => {
            compile_expr(a, c);
            c.emit(Op::Ceil);
        }
        RustExpr::Round(a) => {
            compile_expr(a, c);
            c.emit(Op::Round);
        }
        RustExpr::Exp(a) => {
            compile_expr(a, c);
            c.emit(Op::Exp);
        }
        RustExpr::Ln(a) => {
            compile_expr(a, c);
            c.emit(Op::Ln);
        }
        RustExpr::Log10(a) => {
            compile_expr(a, c);
            c.emit(Op::Log10);
        }
        RustExpr::Log2(a) => {
            compile_expr(a, c);
            c.emit(Op::Log2);
        }
        RustExpr::Sign(a) => {
            compile_expr(a, c);
            c.emit(Op::Sign);
        }
        RustExpr::Fract(a) => {
            compile_expr(a, c);
            c.emit(Op::Fract);
        }
        RustExpr::Mod(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Mod);
        }
        RustExpr::Pow(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Pow);
        }
        RustExpr::Min(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Min);
        }
        RustExpr::Max(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Max);
        }
        RustExpr::Clamp(val, min, max) => {
            compile_expr(val, c);
            compile_expr(min, c);
            compile_expr(max, c);
            c.emit(Op::Clamp);
        }
        RustExpr::Lerp(a, b, t) => {
            compile_expr(a, c);
            compile_expr(b, c);
            compile_expr(t, c);
            c.emit(Op::Lerp);
        }
        // Comparison/logical ops — compile to 1.0/0.0 result
        RustExpr::Eq(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Eq);
        }
        RustExpr::Ne(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Ne);
        }
        RustExpr::Lt(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Lt);
        }
        RustExpr::Le(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Le);
        }
        RustExpr::Gt(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Gt);
        }
        RustExpr::Ge(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Ge);
        }
        RustExpr::And(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::And);
        }
        RustExpr::Or(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::Or);
        }
        RustExpr::Not(a) => {
            compile_expr(a, c);
            c.emit(Op::Not);
        }
        RustExpr::Where(cond, true_expr, false_expr) => {
            compile_expr(cond, c);
            compile_expr(true_expr, c);
            compile_expr(false_expr, c);
            c.emit(Op::Where);
        }
        RustExpr::Random => {
            c.emit(Op::Random);
        }
        RustExpr::RandomRange(min, max) => {
            compile_expr(min, c);
            compile_expr(max, c);
            c.emit(Op::RandomRange);
        }
    }
}

/// Execute a single-component assignment via Bevy's query system.
///
/// Optimized path for the common case where all bytecode fields reference
/// the same component. Uses `QueryBuilder` + `par_iter_mut` for parallel
/// execution with proper Bevy change detection.
///
/// Prefer this over `execute_batch_assignment` for single-component views —
/// it avoids `gather_table_batches` overhead and parallelizes automatically.
pub fn execute_query_assignment(
    world: &mut World,
    component_id: ComponentId,
    filter: &ViewFilter,
    bytecode: &CompiledBytecode,
) {
    let mut qb = QueryBuilder::<FilteredEntityMut>::new(world);
    qb.mut_id(component_id);
    for &id in &filter.with_ids {
        qb.with_id(id);
    }
    for &id in &filter.without_ids {
        qb.without_id(id);
    }
    // Data components that aren't the target still need read access
    for &id in &filter.component_ids {
        if id != component_id {
            qb.ref_id(id);
        }
    }
    let mut qs = qb.build();

    qs.par_iter_mut(world).for_each(|mut em| {
        if let Some(mut untyped) = em.get_mut_by_id(component_id) {
            let ptr = untyped.as_mut().as_ptr();
            unsafe { execute_on_ptr(ptr, bytecode) };
        }
    });
}

/// Cached view execution context.
///
/// Caches table batches and component strides across multiple assignments
/// within the same frame. Create once per system execution, reuse for all
/// View assignments.
pub struct ViewExecutionContext {
    pub batches: Vec<TableBatch>,
    pub strides: HashMap<ComponentId, usize>,
}

impl ViewExecutionContext {
    /// Create a new execution context by gathering batches and resolving strides.
    ///
    /// `last_run` and `this_run` are the system's change-detection ticks,
    /// used to build per-entity tick masks for `Changed`/`Added` filters.
    pub fn new(
        world: &mut World,
        filter: &ViewFilter,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self, ViewEngineError> {
        let strides = resolve_component_strides(world, &filter.component_ids)?;
        let batches = gather_table_batches(world, filter, last_run, this_run);
        Ok(Self { batches, strides })
    }

    /// Execute a pre-compiled bytecode assignment using cached batches/strides.
    ///
    /// Automatically selects the filtered execution path when any batch has a
    /// tick mask (from `Changed`/`Added` filters), or the fast batch path otherwise.
    ///
    /// # Safety
    ///
    /// World must not be structurally modified since this context was created.
    pub unsafe fn execute(
        &self,
        world: &mut World,
        bytecode: &CompiledBytecode,
        dest_component_id: ComponentId,
    ) {
        let has_tick_masks = self.batches.iter().any(|b| b.tick_mask.is_some());
        unsafe {
            if has_tick_masks {
                execute_filtered_assignment(
                    &self.batches,
                    bytecode,
                    &self.strides,
                    cfg!(feature = "parallel"),
                );
            } else {
                execute_batch_assignment(
                    &self.batches,
                    bytecode,
                    &self.strides,
                    cfg!(feature = "parallel"),
                );
            }
        }
        mark_component_changed(world, &self.batches, dest_component_id);
    }

    pub fn entity_count(&self) -> usize {
        self.batches.iter().map(|b| b.entity_count).sum()
    }
}
