//! Shared batch execution engine for the View API.
//!
//! Handles archetype iteration, field pointer resolution, and bytecode
//! execution across entity batches. Python-agnostic — works only with
//! bevy_ecs types and the bytecodevm's CompiledBytecode/VM.

use std::collections::{HashMap, HashSet};

use bevy_ecs::{
    change_detection::Tick,
    component::ComponentId,
    storage::{Table, TableId},
    world::World,
};
use smallvec::SmallVec;

use crate::bytecode::{CompiledBytecode, VM};

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
