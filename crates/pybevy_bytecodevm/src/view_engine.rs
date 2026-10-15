//! Shared batch execution engine for the View API.
//!
//! Handles archetype iteration, field pointer resolution, and bytecode
//! execution across entity batches. Python-agnostic - works only with
//! bevy_ecs types and the bytecodevm's CompiledBytecode/VM.

use std::collections::{HashMap, HashSet};

use bevy_ecs::{
    archetype::Archetypes,
    change_detection::Tick,
    component::{ComponentId, StorageType},
    entity::Entity,
    storage::{Table, TableId, TableRow, Tables},
    world::{World, unsafe_world_cell::UnsafeWorldCell},
};
#[cfg(feature = "parallel")]
use bevy_tasks::ComputeTaskPool;
use nonmax::NonMaxU32;

use crate::{
    bytecode::{CompiledBytecode, Compiler, FieldId, FieldType, NegativeIntegerPower, Op, VM},
    expr::RustExpr,
    tiled::{TiledScratch, execute_assignment_tiled, supported_program},
};

/// A contiguous range of rows in one ECS table.
///
/// Sparse-set components can split one table across multiple archetypes. View
/// filters therefore select table rows, not whole tables. Keeping the selected
/// rows as maximal contiguous ranges preserves the batch VM's contiguous-memory
/// fast path and the public View API's zero-copy column semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableRowRange {
    /// The table containing the selected rows.
    pub table_id: TableId,
    /// First selected row in the table.
    pub start_row: usize,
    /// Number of consecutive selected rows.
    pub entity_count: usize,
}

/// A contiguous batch of selected table rows with raw column pointers.
pub struct TableBatch {
    /// The table this batch was gathered from.
    pub table_id: TableId,
    /// Base pointer for each component's column data in this table.
    pub component_bases: HashMap<ComponentId, *mut u8>,
    /// Row 0 of the table's entity slice, under the same liveness contract
    /// as `component_bases`. Row `r` is `entities.add(r)`.
    pub entities: *const Entity,
    /// First selected row in the table.
    pub start_row: usize,
    /// Number of entities in this contiguous selected range.
    pub entity_count: usize,
    /// Row count of the table when the batch was gathered.
    pub table_entity_count: usize,
    /// Per-entity tick mask. `None` means all entities pass.
    /// `Some(vec)` where `vec[i]` is true if local row i passes all tick filters.
    pub tick_mask: Option<Vec<bool>>,
}

/// Filter criteria for archetype matching.
///
/// All IDs must be pre-resolved before calling view engine functions.
#[derive(Clone, Debug)]
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
    /// A field offset (+ type size) falls outside the component's allocated
    /// layout. This is raised before any raw-pointer arithmetic to prevent
    /// out-of-bounds reads/writes from attacker-controlled `Expr("field", ...)`
    /// nodes constructed on the Python side.
    FieldOffsetOutOfBounds {
        component_id: ComponentId,
        offset: usize,
        type_size: usize,
        layout_size: usize,
    },
    /// A bytecode field references a component the view did not declare.
    /// Reachable when a `field` expression (e.g. a proxy captured from a
    /// different View) names a component absent from this view's filter, whose
    /// column is therefore never gathered. Rejected before execution: the
    /// scheduler was never told about the access (undeclared-read race) and the
    /// stride/base maps lack the id (would otherwise panic on lookup).
    FieldComponentNotDeclared(ComponentId),
    /// A bytecode field's `(offset, field_type)` does not match any real field
    /// of its component. Reachable when a `field` expression supplies a type
    /// that aliases a differently-typed field's bytes (e.g. reading an `f32`
    /// field as `Bool`, whose non-0/1 byte is an invalid bit pattern = instant
    /// UB in `read_field_value`) or an `offset` that lands mid-field. Stricter
    /// than `FieldOffsetOutOfBounds`, which only checks the byte span fits the
    /// layout, not that a field of that type actually lives there.
    FieldTypeMismatch {
        component_id: ComponentId,
        offset: usize,
        field_type: FieldType,
    },
    /// A View data component uses sparse-set storage, which cannot be addressed
    /// by the dense table base/stride execution engine.
    SparseDataComponentUnsupported(ComponentId),
    /// A Changed/Added filter reads ticks from a sparse-set component, whose
    /// per-entity ticks do not live in the selected dense table.
    SparseTickComponentUnsupported(ComponentId),
}

impl std::fmt::Display for ViewEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewEngineError::ComponentNotFound(id) => {
                write!(f, "Component {:?} not found in World", id)
            }
            ViewEngineError::FieldOffsetOutOfBounds {
                component_id,
                offset,
                type_size,
                layout_size,
            } => {
                write!(
                    f,
                    "Field access at offset {offset} (type size {type_size}) is out of bounds \
                     for component {component_id:?} (layout size {layout_size})"
                )
            }
            ViewEngineError::FieldComponentNotDeclared(id) => {
                write!(
                    f,
                    "Field expression references component {:?}, which is not \
                     declared in this view",
                    id
                )
            }
            ViewEngineError::FieldTypeMismatch {
                component_id,
                offset,
                field_type,
            } => {
                write!(
                    f,
                    "Field access at offset {offset} as {field_type:?} does not match any real \
                     field of component {component_id:?} (type confusion / mid-field offset)"
                )
            }
            ViewEngineError::SparseDataComponentUnsupported(component_id) => write!(
                f,
                "View data component {component_id:?} uses sparse-set storage, which is not \
                 supported by the dense table View engine"
            ),
            ViewEngineError::SparseTickComponentUnsupported(component_id) => write!(
                f,
                "View Changed/Added filter component {component_id:?} uses sparse-set storage, \
                 whose per-entity ticks are not supported by the dense table View engine"
            ),
        }
    }
}

impl std::error::Error for ViewEngineError {}

/// Why batch execution stopped or reported failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchExecutionError {
    /// An integer was raised to a negative integer power; see the executing
    /// function for which rows may already have been written.
    NegativeIntegerPower,
    /// A bytecode field names a component with no gathered stride or base.
    /// Nothing was executed.
    FieldComponentNotGathered(ComponentId),
}

impl std::fmt::Display for BatchExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NegativeIntegerPower => NegativeIntegerPower.fmt(f),
            Self::FieldComponentNotGathered(id) => write!(
                f,
                "Field expression references component {id:?}, which was not gathered for this batch"
            ),
        }
    }
}

impl std::error::Error for BatchExecutionError {}

impl From<NegativeIntegerPower> for BatchExecutionError {
    fn from(_: NegativeIntegerPower) -> Self {
        Self::NegativeIntegerPower
    }
}

/// Validate that every field referenced by `bytecode` (including the
/// destination field of an assignment) lies entirely within its
/// component's registered `Layout`.
///
/// This is the safety gate for the raw-pointer arithmetic in
/// `execute_batch_assignment` / `execute_filtered_assignment` /
/// `evaluate_batch_program`,
/// which compute `base.add(field_id.offset)`. Without this check, a
/// Python-constructed `Expr("field", [cid, name, offset, field_type])` with
/// an arbitrary `offset` could read/write out of bounds of the component
/// column allocation.
///
/// Returns `Err` if any field is out of bounds or references a component
/// not present in the World's registry.
pub fn validate_bytecode_offsets(
    world: &World,
    bytecode: &CompiledBytecode,
) -> Result<(), ViewEngineError> {
    let components = world.components();
    for field_id in &bytecode.field_map {
        let info = components
            .get_info(field_id.component_id)
            .ok_or(ViewEngineError::ComponentNotFound(field_id.component_id))?;
        let layout_size = info.layout().size();
        let type_size = field_id.field_type.size_bytes();
        // `offset` and `type_size` are attacker-controlled (from Python).
        // Guard against overflow before the addition, then require the full
        // field span to fit within the component layout.
        let span = field_id.offset.checked_add(type_size).ok_or(
            ViewEngineError::FieldOffsetOutOfBounds {
                component_id: field_id.component_id,
                offset: field_id.offset,
                type_size,
                layout_size,
            },
        )?;
        if span > layout_size {
            return Err(ViewEngineError::FieldOffsetOutOfBounds {
                component_id: field_id.component_id,
                offset: field_id.offset,
                type_size,
                layout_size,
            });
        }
    }
    Ok(())
}

/// Validate that every component referenced by `bytecode` is present in the
/// view's declared component set (`allowed`).
///
/// This is the safety gate paired with `validate_bytecode_offsets`: offsets
/// guard against out-of-bounds within a component, this guards against
/// referencing a component the view never declared. A `field` expression can
/// carry any `ComponentId` (component ids are small enumerable integers, and a
/// field proxy captured from another View can be spliced into an assignment
/// through this one). Without this check the source column is read with access
/// the scheduler was never told about (a data race under the multi-threaded
/// executor) and the stride/base maps miss the id (a panic on lookup).
pub fn validate_bytecode_components(
    bytecode: &CompiledBytecode,
    allowed: &HashSet<ComponentId>,
) -> Result<(), ViewEngineError> {
    for field_id in &bytecode.field_map {
        if !allowed.contains(&field_id.component_id) {
            return Err(ViewEngineError::FieldComponentNotDeclared(
                field_id.component_id,
            ));
        }
    }
    Ok(())
}

/// Validate that every field referenced by `bytecode` names a REAL field of its
/// component: each `(offset, field_type)` must be a member of `allowed_fields`
/// for that component id.
///
/// `allowed_fields` maps each declared component to its set of legitimate
/// `(offset, FieldType)` pairs, with `Vec2`/`Vec3`/`Vec4` fields already expanded
/// to their `F32` sub-fields (the only forms the VM ever emits: the Python `Expr`
/// compiler decomposes vectors into per-lane `F32` reads and never emits a `Vec*`
/// `FieldType` into `field_map`).
///
/// This is the safety gate paired with `validate_bytecode_offsets`. Offsets guard
/// against reads that spill outside the component layout; this guards against a
/// `field` expression whose `field_type` aliases a differently-typed field's bytes
/// or whose `offset` lands mid-field - both in-bounds, so the offset check passes.
/// The sharpest case it closes: an attacker-constructed
/// `Expr("field", [cid, name, offset, "Bool"])` pointed at an `f32`/padding byte,
/// which `read_field_value` reads as `bool` - a non-0/1 byte is an invalid bit
/// pattern and instant undefined behaviour.
pub fn validate_bytecode_field_types(
    bytecode: &CompiledBytecode,
    allowed_fields: &HashMap<ComponentId, HashSet<(usize, FieldType)>>,
) -> Result<(), ViewEngineError> {
    for field_id in &bytecode.field_map {
        let fields = allowed_fields.get(&field_id.component_id).ok_or(
            ViewEngineError::FieldComponentNotDeclared(field_id.component_id),
        )?;
        if !fields.contains(&(field_id.offset, field_id.field_type)) {
            return Err(ViewEngineError::FieldTypeMismatch {
                component_id: field_id.component_id,
                offset: field_id.offset,
                field_type: field_id.field_type,
            });
        }
    }
    Ok(())
}

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
    table_entity_count: usize,
    row_range: TableRowRange,
    changed_ids: &[ComponentId],
    added_ids: &[ComponentId],
    last_run: Tick,
    this_run: Tick,
) -> Option<Vec<bool>> {
    if changed_ids.is_empty() && added_ids.is_empty() {
        return None;
    }

    let mut mask = vec![true; row_range.entity_count];

    for &id in changed_ids {
        if let Some(column) = table.get_column(id) {
            // SAFETY: `table_entity_count` is the table's current row count and
            // the column belongs to this table.
            let changed_ticks = unsafe { column.get_changed_ticks_slice(table_entity_count) };
            for i in 0..row_range.entity_count {
                if mask[i] {
                    // SAFETY: the range was built from live archetype rows and
                    // the tick slice covers the full current table.
                    let tick = unsafe { *changed_ticks[row_range.start_row + i].get() };
                    if !tick.is_newer_than(last_run, this_run) {
                        mask[i] = false;
                    }
                }
            }
        }
    }

    for &id in added_ids {
        if let Some(column) = table.get_column(id) {
            // SAFETY: `table_entity_count` is the table's current row count and
            // the column belongs to this table.
            let added_ticks = unsafe { column.get_added_ticks_slice(table_entity_count) };
            for i in 0..row_range.entity_count {
                if mask[i] {
                    // SAFETY: the range was built from live archetype rows and
                    // the tick slice covers the full current table.
                    let tick = unsafe { *added_ticks[row_range.start_row + i].get() };
                    if !tick.is_newer_than(last_run, this_run) {
                        mask[i] = false;
                    }
                }
            }
        }
    }

    Some(mask)
}

/// Resolve the exact table rows selected by the archetype-level portions of a
/// View filter.
///
/// Multiple archetypes can share a table when they differ only by sparse-set
/// components. Rows are grouped by table, sorted, deduplicated defensively, and
/// coalesced into maximal contiguous ranges. Consequently each selected table
/// row appears in exactly one returned range.
/// Does this archetype carry every component the filter selects on?
fn archetype_matches(archetype: &bevy_ecs::archetype::Archetype, filter: &ViewFilter) -> bool {
    filter
        .component_ids
        .iter()
        .all(|id| archetype.contains(*id))
        && filter.with_ids.iter().all(|id| archetype.contains(*id))
        && !filter.without_ids.iter().any(|id| archetype.contains(*id))
        && filter.changed_ids.iter().all(|id| archetype.contains(*id))
        && filter.added_ids.iter().all(|id| archetype.contains(*id))
}

fn matching_table_row_ranges_from_archetypes(
    archetypes: &Archetypes,
    tables: &Tables,
    filter: &ViewFilter,
) -> Vec<TableRowRange> {
    // A table whose every row is selected needs no per-entity work: the range is
    // the whole table. Tally the selection per table first and only walk entities
    // for tables that are partially selected.
    let mut selected_per_table = Vec::<(TableId, usize)>::new();
    for archetype in archetypes.iter() {
        if !archetype_matches(archetype, filter) {
            continue;
        }
        let table_id = archetype.table_id();
        let count = archetype.entities().len();
        match selected_per_table
            .iter_mut()
            .find(|(existing, _)| *existing == table_id)
        {
            Some((_, total)) => *total += count,
            None => selected_per_table.push((table_id, count)),
        }
    }

    let mut ranges = Vec::new();
    let mut partial_tables = Vec::<TableId>::new();
    for (table_id, selected) in selected_per_table {
        if selected == 0 {
            continue;
        }
        let table_rows = tables
            .get(table_id)
            .map(|table| table.entity_count() as usize);
        if table_rows == Some(selected) {
            ranges.push(TableRowRange {
                table_id,
                start_row: 0,
                entity_count: selected,
            });
        } else {
            partial_tables.push(table_id);
        }
    }
    if partial_tables.is_empty() {
        return ranges;
    }

    let mut rows_by_table = Vec::<(TableId, Vec<usize>)>::new();

    for archetype in archetypes.iter() {
        if !archetype_matches(archetype, filter) {
            continue;
        }

        let table_id = archetype.table_id();
        if !partial_tables.contains(&table_id) {
            continue;
        }
        let index = rows_by_table
            .iter()
            .position(|(existing_id, _)| *existing_id == table_id)
            .unwrap_or_else(|| {
                let index = rows_by_table.len();
                rows_by_table.push((table_id, Vec::new()));
                index
            });
        rows_by_table[index].1.extend(
            archetype
                .entities()
                .iter()
                .map(|entity| entity.table_row().index()),
        );
    }

    for (table_id, mut rows) in rows_by_table {
        rows.sort_unstable();
        rows.dedup();

        let Some(&first) = rows.first() else {
            continue;
        };
        let mut start = first;
        let mut previous = first;
        for &row in &rows[1..] {
            if row == previous + 1 {
                previous = row;
                continue;
            }
            ranges.push(TableRowRange {
                table_id,
                start_row: start,
                entity_count: previous - start + 1,
            });
            start = row;
            previous = row;
        }
        ranges.push(TableRowRange {
            table_id,
            start_row: start,
            entity_count: previous - start + 1,
        });
    }
    ranges
}

/// Resolve exact selected rows using an ordinary shared World reference.
pub fn matching_table_row_ranges(world: &World, filter: &ViewFilter) -> Vec<TableRowRange> {
    matching_table_row_ranges_from_archetypes(world.archetypes(), &world.storages().tables, filter)
}

/// Resolve exact selected rows from World metadata without borrowing the World.
///
/// # Safety
///
/// `world_cell` must permit reading table metadata for the duration of the call.
pub unsafe fn matching_table_row_ranges_from_cell(
    world_cell: UnsafeWorldCell<'_>,
    filter: &ViewFilter,
) -> Vec<TableRowRange> {
    // SAFETY: guaranteed by the caller; this reads table metadata only.
    let storages = unsafe { world_cell.storages() };
    matching_table_row_ranges_from_archetypes(world_cell.archetypes(), &storages.tables, filter)
}

/// Gather exact dense-table batches through access-bounded World-cell storage.
///
/// Sparse-set data components and sparse Changed/Added tick sources are
/// rejected rather than silently producing incomplete or incorrectly filtered
/// results.
///
/// # Safety
///
/// - `world_cell` must remain live for every returned pointer use.
/// - The scheduler's declared access must cover every data and tick component
///   in `filter`, including mutable access for any later destination writes.
/// - No structural World mutation may overlap the returned batches.
/// - Callers must prevent aliasing accesses to the same selected component rows.
pub unsafe fn gather_table_batches_from_cell(
    world_cell: UnsafeWorldCell<'_>,
    filter: &ViewFilter,
    last_run: Tick,
    this_run: Tick,
) -> Result<Vec<TableBatch>, ViewEngineError> {
    let has_tick_filters = !filter.changed_ids.is_empty() || !filter.added_ids.is_empty();

    let components = world_cell.components();
    for &component_id in &filter.component_ids {
        let info = components
            .get_info(component_id)
            .ok_or(ViewEngineError::ComponentNotFound(component_id))?;
        if matches!(info.storage_type(), StorageType::SparseSet) {
            return Err(ViewEngineError::SparseDataComponentUnsupported(
                component_id,
            ));
        }
    }
    for &component_id in filter.changed_ids.iter().chain(&filter.added_ids) {
        let info = components
            .get_info(component_id)
            .ok_or(ViewEngineError::ComponentNotFound(component_id))?;
        if matches!(info.storage_type(), StorageType::SparseSet) {
            return Err(ViewEngineError::SparseTickComponentUnsupported(
                component_id,
            ));
        }
    }

    // SAFETY: guaranteed by the caller. This obtains only the storage registry;
    // the code below reads table metadata and the exact declared columns/ticks.
    let storages = unsafe { world_cell.storages() };
    let tables = &storages.tables;

    let mut batches = Vec::new();
    // SAFETY: guaranteed by this function's own contract.
    for row_range in unsafe { matching_table_row_ranges_from_cell(world_cell, filter) } {
        let Some(table) = tables.get(row_range.table_id) else {
            continue;
        };
        let table_entity_count = table.entity_count() as usize;
        let Some(range_end) = row_range.start_row.checked_add(row_range.entity_count) else {
            continue;
        };
        if row_range.entity_count == 0 || range_end > table_entity_count {
            continue;
        }

        for &component_id in filter.changed_ids.iter().chain(&filter.added_ids) {
            if table.get_column(component_id).is_none() {
                return Err(ViewEngineError::SparseTickComponentUnsupported(
                    component_id,
                ));
            }
        }

        let tick_mask = if has_tick_filters {
            build_tick_mask_for_table(
                table,
                table_entity_count,
                row_range,
                &filter.changed_ids,
                &filter.added_ids,
                last_run,
                this_run,
            )
        } else {
            None
        };

        if let Some(ref mask) = tick_mask
            && !mask.iter().any(|&passes| passes)
        {
            continue;
        }

        let mut component_bases = HashMap::with_capacity(filter.component_ids.len());
        for &component_id in &filter.component_ids {
            // Not `get_data_slice::<u8>`: that slice's provenance covers only
            // its first `table_entity_count` bytes, not the rows read below.
            // SAFETY: the range check above proves the table holds row 0, and
            // the caller owns scheduler access to this declared component for
            // the returned pointer's lifetime.
            let pointer =
                unsafe { table.get_component(component_id, TableRow::new(NonMaxU32::ZERO)) }
                    .ok_or(ViewEngineError::SparseDataComponentUnsupported(
                        component_id,
                    ))?
                    .as_ptr();
            component_bases.insert(component_id, pointer);
        }

        batches.push(TableBatch {
            table_id: row_range.table_id,
            component_bases,
            entities: table.entities().as_ptr(),
            start_row: row_range.start_row,
            entity_count: row_range.entity_count,
            table_entity_count,
            tick_mask,
        });
    }
    Ok(batches)
}

/// Seed `Op::Random` from entity identity, so the value survives the entity
/// moving table or row and is the same on every execution path.
#[inline]
pub fn entity_random_seed(entity: Entity) -> u64 {
    let mut bits = entity.to_bits();
    bits = (bits ^ (bits >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    bits = (bits ^ (bits >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    bits ^ (bits >> 31)
}

/// Seed for one selected row of `batch`.
///
/// # Safety
/// `table_row` must be one of this batch's selected rows, and the batch's
/// `entities` pointer must still satisfy its liveness contract.
#[inline]
unsafe fn batch_row_seed(batch: &TableBatch, table_row: usize) -> u64 {
    // SAFETY: the caller guarantees `table_row` is inside the live entity slice.
    entity_random_seed(unsafe { *batch.entities.add(table_row) })
}

/// Whether a program's random values depend on the row seed.
#[inline]
fn program_uses_random(bytecode: &CompiledBytecode) -> bool {
    bytecode
        .bytecode
        .iter()
        .any(|op| matches!(op, Op::Random(_) | Op::RandomRange(_)))
}

/// Maximum rows handed to one executor call and the unit of parallel work.
const RUN_ROWS: usize = 32768;

/// One contiguous run of selected rows inside `batches[batch]`.
#[derive(Clone, Copy)]
struct RowRun {
    batch: usize,
    table_row: usize,
    count: usize,
}

/// Selected rows as ordered contiguous runs of at most [`RUN_ROWS`] rows.
/// Tick masks select rows only when `honor_masks` is set.
fn row_runs(batches: &[TableBatch], honor_masks: bool) -> Vec<RowRun> {
    let mut runs = Vec::new();
    for (index, batch) in batches.iter().enumerate() {
        let mut push = |local_start: usize, local_end: usize| {
            let mut start = local_start;
            while start < local_end {
                let count = (local_end - start).min(RUN_ROWS);
                runs.push(RowRun {
                    batch: index,
                    table_row: batch.start_row + start,
                    count,
                });
                start += count;
            }
        };
        match batch.tick_mask.as_ref().filter(|_| honor_masks) {
            None => push(0, batch.entity_count),
            Some(mask) => {
                let mut row = 0;
                while row < batch.entity_count {
                    if !mask[row] {
                        row += 1;
                        continue;
                    }
                    let start = row;
                    while row < batch.entity_count && mask[row] {
                        row += 1;
                    }
                    push(start, row);
                }
            }
        }
    }
    runs
}

/// Batches shared with compute-pool tasks, which address disjoint row runs.
#[derive(Clone, Copy)]
struct SharedBatches<'a>(&'a [TableBatch]);
// SAFETY: tasks read only batch metadata and dereference pointers for the
// disjoint row runs they were handed, under the caller's execution contract.
unsafe impl Send for SharedBatches<'_> {}
// SAFETY: see the `Send` impl; no task mutates the batch metadata.
unsafe impl Sync for SharedBatches<'_> {}

impl SharedBatches<'_> {
    fn get(&self) -> &[TableBatch] {
        self.0
    }
}

/// Field base pointers for the first row of `run`, in field-map order.
///
/// # Safety
/// `run` must lie inside its batch's gathered range and every field's
/// component must have a base in that batch.
unsafe fn run_bases(
    batch: &TableBatch,
    bytecode: &CompiledBytecode,
    field_strides: &[usize],
    run: RowRun,
    bases: &mut Vec<*mut u8>,
) {
    bases.clear();
    bases.extend(
        bytecode
            .field_map
            .iter()
            .zip(field_strides)
            .map(|(field, stride)| {
                // SAFETY: the run is inside the gathered column range and the
                // validated field offset fits the component layout.
                unsafe {
                    batch.component_bases[&field.component_id]
                        .add(field.offset)
                        .add(run.table_row * stride)
                }
            }),
    );
}

/// Seeds for every row of `run`, from each row's entity.
///
/// # Safety
/// `run` must lie inside its batch's gathered range.
unsafe fn run_seeds(batch: &TableBatch, run: RowRun, seeds: &mut Vec<u64>) {
    seeds.clear();
    seeds.extend((0..run.count).map(|row| {
        // SAFETY: every row of the run is inside the gathered entity slice.
        unsafe { batch_row_seed(batch, run.table_row + row) }
    }));
}

/// Per-field strides, rejecting a field whose component was not gathered.
fn field_strides(
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
) -> Result<Vec<usize>, BatchExecutionError> {
    bytecode
        .field_map
        .iter()
        .map(|field| {
            component_strides.get(&field.component_id).copied().ok_or(
                BatchExecutionError::FieldComponentNotGathered(field.component_id),
            )
        })
        .collect()
}

/// Execute bytecode assignment across all entities in all batches (fast path).
///
/// Tick masks are ignored; use [`execute_filtered_assignment`] for masked
/// batches. When `parallel` is true (and the `parallel` feature is enabled),
/// uses Bevy's compute pool for chunked execution if that pool is initialized.
///
/// # Safety
///
/// All pointers in `batches` must be valid for the duration of this call.
/// No other code may mutate the same memory concurrently.
///
/// An integer raised to a negative power is reported after the single pass,
/// so every chunk, including rows after the failing one, may have been written.
pub unsafe fn execute_batch_assignment(
    batches: &[TableBatch],
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
    parallel: bool,
) -> Result<(), BatchExecutionError> {
    // SAFETY: forwarded from this function's contract.
    unsafe { execute_runs(batches, bytecode, component_strides, parallel, false) }
}

/// Execute bytecode assignment on tick-filtered entities only.
///
/// Each maximal run of rows passing the tick mask executes like an unfiltered
/// batch, so filtered and unfiltered assignments share one executor.
///
/// # Safety
///
/// All pointers in `batches` must be valid. Tick masks must accurately
/// reflect which entities should be processed.
///
/// A negative integer power is reported after every selected row is processed.
pub unsafe fn execute_filtered_assignment(
    batches: &[TableBatch],
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
    parallel: bool,
) -> Result<(), BatchExecutionError> {
    // SAFETY: forwarded from this function's contract.
    unsafe { execute_runs(batches, bytecode, component_strides, parallel, true) }
}

/// # Safety
/// As [`execute_batch_assignment`].
unsafe fn execute_runs(
    batches: &[TableBatch],
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
    parallel: bool,
    honor_masks: bool,
) -> Result<(), BatchExecutionError> {
    let field_strides = field_strides(bytecode, component_strides)?;
    let runs = row_runs(batches, honor_masks);
    let tiled = supported_program(bytecode);
    let needs_seeds = program_uses_random(bytecode);
    let shared = SharedBatches(batches);
    let field_strides = field_strides.as_slice();

    let process = move |group: &[RowRun]| {
        let batches = shared.get();
        let mut scratch = tiled.then(TiledScratch::new);
        let mut vm = VM::new();
        let mut bases = Vec::with_capacity(field_strides.len());
        let mut seeds = Vec::new();
        for &run in group {
            let batch = &batches[run.batch];
            // SAFETY: runs come from the gathered ranges of `batches`.
            unsafe { run_bases(batch, bytecode, field_strides, run, &mut bases) };
            if let Some(scratch) = scratch.as_mut() {
                // SAFETY: the bases and strides span exactly this run's rows.
                unsafe {
                    execute_assignment_tiled(bytecode, &bases, field_strides, run.count, scratch)
                }
                .expect("supported_program checked above");
                continue;
            }
            if needs_seeds {
                // SAFETY: as above.
                unsafe { run_seeds(batch, run, &mut seeds) };
            }
            // SAFETY: the bases and strides span exactly this run's rows,
            // which no other task addresses.
            unsafe {
                vm.execute_batch_multi(
                    bytecode,
                    &bases,
                    field_strides,
                    run.count,
                    needs_seeds.then_some(seeds.as_slice()),
                );
            }
        }
        vm.take_negative_power().map_err(BatchExecutionError::from)
    };

    #[cfg(feature = "parallel")]
    if parallel
        && runs.len() > 1
        && let Some(pool) = ComputeTaskPool::try_get()
        && pool.thread_num() > 1
    {
        let task_size = runs.len().div_ceil(pool.thread_num());
        let outcomes = pool.scope(|scope| {
            for group in runs.chunks(task_size) {
                scope.spawn(async move { process(group) });
            }
        });
        return outcomes.into_iter().collect();
    }

    let _ = parallel;
    process(&runs)
}

/// Evaluate one validated read-only program over exact gathered batches.
///
/// This is crate-private so raw batch pointers cannot bypass
/// `ViewRuntimeCore`'s validation and operation fence. Results preserve batch
/// and row order even when evaluation runs in parallel.
///
/// # Safety
///
/// - Every bytecode field must have been validated against the gathered
///   component bases and `component_strides`.
/// - All batch pointers and tick masks must remain live and unchanged for the
///   complete call.
/// - Scheduler access must prevent writes to every referenced component.
pub(crate) unsafe fn evaluate_batch_program(
    batches: &[TableBatch],
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
    parallel: bool,
) -> Result<Vec<f64>, BatchExecutionError> {
    let field_strides = field_strides(bytecode, component_strides)?;
    let runs = row_runs(batches, true);
    let needs_seeds = program_uses_random(bytecode);
    let shared = SharedBatches(batches);
    let field_strides = field_strides.as_slice();

    let evaluate = move |group: &[RowRun]| {
        let batches = shared.get();
        let mut vm = VM::new();
        let mut bases = Vec::with_capacity(field_strides.len());
        let mut row_pointers = Vec::with_capacity(field_strides.len());
        let mut values = Vec::with_capacity(group.iter().map(|run| run.count).sum());
        for &run in group {
            let batch = &batches[run.batch];
            // SAFETY: runs come from the gathered ranges of `batches`.
            unsafe { run_bases(batch, bytecode, field_strides, run, &mut bases) };
            for row in 0..run.count {
                row_pointers.clear();
                row_pointers.extend(bases.iter().zip(field_strides).map(|(base, stride)| {
                    // SAFETY: `row` is inside this run's gathered rows.
                    unsafe { base.add(row * stride) }
                }));
                let seed = if needs_seeds {
                    // SAFETY: `row` is inside this run's gathered rows.
                    unsafe { batch_row_seed(batch, run.table_row + row) }
                } else {
                    0
                };
                // SAFETY: upheld by this function's caller; the bytecode is
                // read-only and fully validated.
                values.push(unsafe { vm.execute_and_reduce(bytecode, &row_pointers, seed) });
            }
        }
        vm.take_negative_power()
            .map(|()| values)
            .map_err(BatchExecutionError::from)
    };

    #[cfg(feature = "parallel")]
    if parallel
        && runs.len() > 1
        && let Some(pool) = ComputeTaskPool::try_get()
        && pool.thread_num() > 1
    {
        let task_size = runs.len().div_ceil(pool.thread_num());
        let groups = pool.scope(|scope| {
            for (index, group) in runs.chunks(task_size).enumerate() {
                scope.spawn(async move { (index, evaluate(group)) });
            }
        });
        let mut values = vec![Vec::new(); groups.len()];
        for (index, group) in groups {
            values[index] = group?;
        }
        return Ok(values.into_iter().flatten().collect());
    }

    let _ = parallel;
    evaluate(&runs)
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
    // Fold constant arithmetic, as `RustExpr::compile_assignment` does, so
    // the folded value is range-checked and the program stays tiled-eligible.
    compiler.optimize();
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
        RustExpr::Input { index, .. } => {
            c.emit(Op::PushInput(*index));
        }
        RustExpr::Const(v) => {
            let idx = c.add_constant(*v);
            c.emit(Op::PushConst(idx));
        }
        RustExpr::FloatConst(v) => {
            let idx = c.add_constant(*v);
            c.emit(Op::PushFloatConst(idx));
        }
        RustExpr::Int(v) => {
            let push = c.int_literal(*v);
            c.emit(push);
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
        RustExpr::FloorDiv(a, b) => {
            compile_expr(a, c);
            compile_expr(b, c);
            c.emit(Op::FloorDiv);
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
        // Comparison/logical ops - compile to 1.0/0.0 result
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
        RustExpr::Random(stream) => {
            c.emit(Op::Random(*stream));
        }
        RustExpr::RandomRange(min, max, stream) => {
            compile_expr(min, c);
            compile_expr(max, c);
            c.emit(Op::RandomRange(*stream));
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        mem::size_of,
    };

    use bevy_ecs::{component::ComponentId, prelude::Component};
    #[cfg(feature = "parallel")]
    use bevy_tasks::TaskPoolBuilder;

    use super::*;
    use crate::bytecode::{Compiler, FieldId, FieldType, Op};

    /// Fixed-layout probe component: three f32 fields, so `Layout::size() == 12`
    /// and `align == 4`. Used to exercise the offset validator against a real
    /// registered component layout.
    #[derive(Component)]
    #[allow(dead_code)]
    struct Probe {
        a: f32,
        b: f32,
        c: f32,
    }

    #[derive(Component)]
    struct DenseValue(f32);

    #[derive(Component)]
    #[component(storage = "SparseSet")]
    struct SparseMarker;

    /// Synthetic entity rows for batches built over test-owned memory. Leaked
    /// so a batch literal can hold the pointer for the whole test.
    fn test_entities(first: u64, count: usize) -> *const Entity {
        // Entity bit patterns are non-zero, so number the rows from one.
        let entities: Vec<Entity> = (first..first + count as u64)
            .map(|index| Entity::from_bits(index + 1))
            .collect();
        Box::leak(entities.into_boxed_slice()).as_ptr()
    }

    fn sparse_filter(component_id: ComponentId, marker_id: ComponentId, with: bool) -> ViewFilter {
        ViewFilter {
            component_ids: HashSet::from([component_id]),
            with_ids: if with { vec![marker_id] } else { Vec::new() },
            without_ids: if with { Vec::new() } else { vec![marker_id] },
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        }
    }

    /// Helper: bytecode that reads a single field of the given type at `offset`.
    fn make_typed_read_bytecode(
        component_id: ComponentId,
        offset: usize,
        field_type: FieldType,
    ) -> CompiledBytecode {
        let mut compiler = Compiler::new();
        let field_idx = compiler.add_field(FieldId {
            component_id,
            offset,
            field_type,
        });
        compiler.emit(Op::PushField(field_idx));
        compiler.finalize()
    }

    /// Helper: compile `field[0] = field[0] + constant`
    fn make_add_bytecode(
        component_id: ComponentId,
        offset: usize,
        constant: f64,
    ) -> CompiledBytecode {
        let mut compiler = Compiler::new();
        let field_id = FieldId {
            component_id,
            offset,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_idx = compiler.add_constant(constant);
        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_idx));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));
        compiler.finalize()
    }

    #[test]
    fn test_resolve_component_strides_missing() {
        let world = World::new();
        let mut ids = HashSet::new();
        ids.insert(ComponentId::new(9999));
        let result = resolve_component_strides(&world, &ids);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_batch_assignment_serial() {
        let cid = ComponentId::new(0);
        let bytecode = make_add_bytecode(cid, 0, 1.0);

        // Simulate 3 entities, each an f32 (stride=4)
        let mut data = [0.0_f32; 3];
        data[0] = 10.0;
        data[1] = 20.0;
        data[2] = 30.0;

        let base = data.as_mut_ptr() as *mut u8;
        let mut component_bases = HashMap::new();
        component_bases.insert(cid, base);

        let batches = vec![TableBatch {
            table_id: TableId::from_u32(0),
            component_bases,
            entities: test_entities(0, 3),
            start_row: 0,
            entity_count: 3,
            table_entity_count: 3,
            tick_mask: None,
        }];

        let mut strides = HashMap::new();
        strides.insert(cid, 4usize); // f32 stride

        unsafe { execute_batch_assignment(&batches, &bytecode, &strides, false) }.unwrap();

        assert_eq!(data[0], 11.0);
        assert_eq!(data[1], 21.0);
        assert_eq!(data[2], 31.0);
    }

    #[test]
    fn contiguous_iter_oracle_tiled_view_assignment() {
        let mut world = World::new();
        let cid = world.register_component::<DenseValue>();
        let orig: Vec<f32> = (0..300).map(|i| i as f32 * 0.5 - 40.0).collect();
        for &v in &orig {
            world.spawn(DenseValue(v));
        }

        let filter = ViewFilter {
            component_ids: HashSet::from([cid]),
            with_ids: Vec::new(),
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        };
        let bytecode = make_add_bytecode(cid, 0, 2.5);
        assert!(supported_program(&bytecode));
        // SAFETY: the test owns `world` exclusively for the gather and the
        // execution below, and no structural change happens in between.
        let batches = unsafe {
            gather_table_batches_from_cell(
                world.as_unsafe_world_cell(),
                &filter,
                Tick::new(0),
                Tick::new(1),
            )
        }
        .unwrap();
        let strides = resolve_component_strides(&world, &filter.component_ids).unwrap();
        // SAFETY: batches point into live, exclusively-held table columns.
        unsafe { execute_batch_assignment(&batches, &bytecode, &strides, false) }.unwrap();

        // Read through Bevy's dense contiguous query API.
        let mut state = world.query::<&DenseValue>();
        let mut got: Vec<f32> = Vec::new();
        for slice in state
            .contiguous_iter(&world)
            .expect("dense query is contiguous")
        {
            got.extend(slice.iter().map(|dv| dv.0));
        }
        // Tiled path widens f32 -> f64, adds, narrows back; mirror that exactly.
        let mut want: Vec<f32> = orig.iter().map(|v| (*v as f64 + 2.5) as f32).collect();
        got.sort_by(f32::total_cmp);
        want.sort_by(f32::total_cmp);
        assert_eq!(
            got, want,
            "tiled View results must match via contiguous_iter"
        );
    }

    #[test]
    fn negative_field_exponent_is_reported_after_the_single_pass() {
        let mut compiler = Compiler::new();
        let component_id = ComponentId::new(0);
        let base = compiler.add_field(FieldId {
            component_id,
            offset: 0,
            field_type: FieldType::I64,
        });
        let exponent = compiler.add_field(FieldId {
            component_id,
            offset: 8,
            field_type: FieldType::I64,
        });
        compiler.emit(Op::PushField(base));
        compiler.emit(Op::PushField(exponent));
        compiler.emit(Op::Pow);
        compiler.emit(Op::StoreField(base));
        let bytecode = compiler.finalize();
        assert!(!supported_program(&bytecode));

        let strides = HashMap::from([(component_id, 2 * size_of::<i64>())]);
        for mask in [None, Some(vec![true; 3])] {
            let mut rows = [[2_i64, 2], [3, -1], [4, 2]];
            let batches = [TableBatch {
                table_id: TableId::from_u32(0),
                component_bases: HashMap::from([(component_id, rows.as_mut_ptr().cast::<u8>())]),
                entities: test_entities(0, rows.len()),
                start_row: 0,
                entity_count: rows.len(),
                table_entity_count: rows.len(),
                tick_mask: mask.clone(),
            }];
            // SAFETY: the batch points to three live, exclusively held rows.
            let result = unsafe {
                if mask.is_some() {
                    execute_filtered_assignment(&batches, &bytecode, &strides, false)
                } else {
                    execute_batch_assignment(&batches, &bytecode, &strides, false)
                }
            };
            assert_eq!(result, Err(BatchExecutionError::NegativeIntegerPower));
            assert_eq!(rows.map(|row| row[0]), [4, 0, 16]);
        }

        let mut rows = [[2_i64, 3], [3, 2]];
        let batches = [TableBatch {
            table_id: TableId::from_u32(0),
            component_bases: HashMap::from([(component_id, rows.as_mut_ptr().cast::<u8>())]),
            entities: test_entities(0, rows.len()),
            start_row: 0,
            entity_count: rows.len(),
            table_entity_count: rows.len(),
            tick_mask: None,
        }];
        // SAFETY: the batch points to two live, exclusively held rows.
        let result = unsafe { execute_batch_assignment(&batches, &bytecode, &strides, false) };
        assert_eq!(result, Ok(()));
        assert_eq!(rows.map(|row| row[0]), [8, 9]);
    }

    #[test]
    fn tiled_view_assignment_supports_homogeneous_integer_fields() {
        let mut compiler = Compiler::new();
        let component_id = ComponentId::new(0);
        let field = compiler.add_field(FieldId {
            component_id,
            offset: 0,
            field_type: FieldType::U64,
        });
        let one = compiler.add_constant(1.0);
        compiler.emit(Op::PushField(field));
        compiler.emit(Op::PushConst(one));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field));
        let bytecode = compiler.finalize();
        assert!(supported_program(&bytecode));

        let mut values = [(1_u64 << 53) + 1, u64::MAX];
        let batches = [TableBatch {
            table_id: TableId::from_u32(0),
            component_bases: HashMap::from([(component_id, values.as_mut_ptr().cast::<u8>())]),
            entities: test_entities(0, values.len()),
            start_row: 0,
            entity_count: values.len(),
            table_entity_count: values.len(),
            tick_mask: None,
        }];
        let strides = HashMap::from([(component_id, size_of::<u64>())]);
        // SAFETY: the batch points to one live, exclusively held u64 run.
        unsafe { execute_batch_assignment(&batches, &bytecode, &strides, false) }.unwrap();

        assert_eq!(values, [(1_u64 << 53) + 2, 0]);
    }

    #[test]
    fn test_execute_filtered_assignment_with_mask() {
        let cid = ComponentId::new(0);
        let bytecode = make_add_bytecode(cid, 0, 100.0);

        let mut data = [0.0_f32; 4];
        data[0] = 1.0;
        data[1] = 2.0;
        data[2] = 3.0;
        data[3] = 4.0;

        let base = data.as_mut_ptr() as *mut u8;
        let mut component_bases = HashMap::new();
        component_bases.insert(cid, base);

        // Only entities 1 and 3 pass the tick filter
        let batches = vec![TableBatch {
            table_id: TableId::from_u32(0),
            component_bases,
            entities: test_entities(0, 4),
            start_row: 0,
            entity_count: 4,
            table_entity_count: 4,
            tick_mask: Some(vec![false, true, false, true]),
        }];

        let mut strides = HashMap::new();
        strides.insert(cid, 4usize);

        unsafe { execute_filtered_assignment(&batches, &bytecode, &strides, false) }.unwrap();

        assert_eq!(data[0], 1.0); // unchanged
        assert_eq!(data[1], 102.0); // +100
        assert_eq!(data[2], 3.0); // unchanged
        assert_eq!(data[3], 104.0); // +100
    }

    #[test]
    fn sparse_filters_select_exact_rows_from_shared_table() {
        let mut world = World::new();
        let plain_first = world.spawn(DenseValue(0.0)).id();
        let selected_first = world.spawn((DenseValue(1.0), SparseMarker)).id();
        let selected_second = world.spawn((DenseValue(2.0), SparseMarker)).id();
        let plain_last = world.spawn(DenseValue(3.0)).id();

        let component_id = world.components().component_id::<DenseValue>().unwrap();
        let marker_id = world.components().component_id::<SparseMarker>().unwrap();

        let with_ranges =
            matching_table_row_ranges(&world, &sparse_filter(component_id, marker_id, true));
        assert_eq!(with_ranges.len(), 1);
        assert_eq!(with_ranges[0].start_row, 1);
        assert_eq!(with_ranges[0].entity_count, 2);

        let table = world
            .storages()
            .tables
            .get(with_ranges[0].table_id)
            .unwrap();
        assert_eq!(&table.entities()[1..3], &[selected_first, selected_second]);

        let without_ranges =
            matching_table_row_ranges(&world, &sparse_filter(component_id, marker_id, false));
        assert_eq!(
            without_ranges,
            vec![
                TableRowRange {
                    table_id: with_ranges[0].table_id,
                    start_row: 0,
                    entity_count: 1,
                },
                TableRowRange {
                    table_id: with_ranges[0].table_id,
                    start_row: 3,
                    entity_count: 1,
                },
            ]
        );
        assert_eq!(table.entities()[0], plain_first);
        assert_eq!(table.entities()[3], plain_last);

        let all_filter = ViewFilter {
            component_ids: HashSet::from([component_id]),
            with_ids: Vec::new(),
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        };
        let all_ranges = matching_table_row_ranges(&world, &all_filter);
        assert_eq!(
            all_ranges,
            vec![TableRowRange {
                table_id: with_ranges[0].table_id,
                start_row: 0,
                entity_count: 4,
            }]
        );
    }

    #[test]
    fn sparse_filtered_assignment_only_mutates_selected_rows() {
        let mut world = World::new();
        let plain_first = world.spawn(DenseValue(10.0)).id();
        let selected_first = world.spawn((DenseValue(20.0), SparseMarker)).id();
        let selected_second = world.spawn((DenseValue(30.0), SparseMarker)).id();
        let plain_last = world.spawn(DenseValue(40.0)).id();

        let component_id = world.components().component_id::<DenseValue>().unwrap();
        let marker_id = world.components().component_id::<SparseMarker>().unwrap();
        let filter = sparse_filter(component_id, marker_id, true);
        let bytecode = make_add_bytecode(component_id, 0, 5.0);
        world.clear_trackers();
        let last_run = world.last_change_tick();
        world.increment_change_tick();
        let this_run = world.change_tick();
        let strides = resolve_component_strides(&world, &filter.component_ids).unwrap();
        // SAFETY: the test owns `world` exclusively for the gather and the
        // execution below, and no structural change happens in between.
        let batches = unsafe {
            gather_table_batches_from_cell(
                world.as_unsafe_world_cell(),
                &filter,
                last_run,
                this_run,
            )
        }
        .unwrap();

        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.entity_count)
                .sum::<usize>(),
            2
        );
        // SAFETY: the batches were gathered from this unchanged World and the
        // bytecode only accesses the declared `DenseValue` component field.
        unsafe { execute_batch_assignment(&batches, &bytecode, &strides, false) }.unwrap();

        assert_eq!(
            world.entity(plain_first).get::<DenseValue>().unwrap().0,
            10.0
        );
        assert_eq!(
            world.entity(selected_first).get::<DenseValue>().unwrap().0,
            25.0
        );
        assert_eq!(
            world.entity(selected_second).get::<DenseValue>().unwrap().0,
            35.0
        );
        assert_eq!(
            world.entity(plain_last).get::<DenseValue>().unwrap().0,
            40.0
        );
    }

    #[test]
    fn execution_rejects_a_field_whose_component_was_not_gathered() {
        let cid = ComponentId::new(0);
        let missing = ComponentId::new(9);
        let bytecode = make_add_bytecode(missing, 0, 1.0);
        let batches: [TableBatch; 0] = [];
        let strides = HashMap::from([(cid, size_of::<f32>())]);
        let expected = Err(BatchExecutionError::FieldComponentNotGathered(missing));

        // SAFETY: no batch is supplied, and the missing component is rejected
        // before any pointer could be formed.
        unsafe {
            assert_eq!(
                execute_batch_assignment(&batches, &bytecode, &strides, false),
                expected
            );
            assert_eq!(
                execute_filtered_assignment(&batches, &bytecode, &strides, false),
                expected
            );
            assert_eq!(
                evaluate_batch_program(&batches, &bytecode, &strides, false).map(|_| ()),
                expected
            );
        }
    }

    #[test]
    fn test_execute_batch_assignment_empty() {
        let cid = ComponentId::new(0);
        let bytecode = make_add_bytecode(cid, 0, 1.0);

        let batches: Vec<TableBatch> = vec![];
        let strides = HashMap::from([(cid, size_of::<f32>())]);

        // Should not panic on empty batches
        unsafe { execute_batch_assignment(&batches, &bytecode, &strides, false) }.unwrap();
    }

    #[test]
    fn random_is_identical_on_every_execution_path() {
        #[cfg(feature = "parallel")]
        assert_eq!(
            ComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(2).build())
                .thread_num(),
            2
        );

        let cid = ComponentId::new(0);
        let table_id = TableId::from_u32(3);
        let strides = HashMap::from([(cid, size_of::<f64>())]);

        let mut compiler = Compiler::new();
        let field = compiler.add_field(FieldId {
            component_id: cid,
            offset: 0,
            field_type: FieldType::F64,
        });
        compiler.emit(Op::Random(0));
        compiler.emit(Op::StoreField(field));
        let assignment = compiler.finalize();

        let mut read_only = Compiler::new();
        read_only.emit(Op::Random(0));
        let read_only = read_only.finalize();

        // More than one 32768-row chunk, so a chunk-local seed would restart.
        let count = 32_770;

        let mut batched = vec![0.0_f64; count];
        let batches = [TableBatch {
            table_id,
            component_bases: HashMap::from([(cid, batched.as_mut_ptr().cast::<u8>())]),
            entities: test_entities(0, count),
            start_row: 0,
            entity_count: count,
            table_entity_count: count,
            tick_mask: None,
        }];
        // SAFETY: the batch describes one live, exclusively held f64 run.
        unsafe { execute_batch_assignment(&batches, &assignment, &strides, false) }.unwrap();
        #[cfg(feature = "parallel")]
        {
            let serial = batched.clone();
            batched.fill(0.0);
            // SAFETY: the same live, exclusive batch is reused after serial execution.
            unsafe { execute_batch_assignment(&batches, &assignment, &strides, true) }.unwrap();
            assert_eq!(batched, serial);
        }

        let mut filtered = vec![0.0_f64; count];
        let batches = [TableBatch {
            table_id,
            component_bases: HashMap::from([(cid, filtered.as_mut_ptr().cast::<u8>())]),
            entities: test_entities(0, count),
            start_row: 0,
            entity_count: count,
            table_entity_count: count,
            tick_mask: Some(vec![true; count]),
        }];
        // SAFETY: as above; the mask selects every row of the same run.
        unsafe { execute_filtered_assignment(&batches, &assignment, &strides, false) }.unwrap();
        #[cfg(feature = "parallel")]
        {
            let serial = filtered.clone();
            filtered.fill(0.0);
            // SAFETY: the same live, exclusive rows are reused after serial execution.
            unsafe { execute_filtered_assignment(&batches, &assignment, &strides, true) }.unwrap();
            assert_eq!(filtered, serial);
        }

        let batches = [TableBatch {
            table_id,
            component_bases: HashMap::new(),
            entities: test_entities(0, count),
            start_row: 0,
            entity_count: count,
            table_entity_count: count,
            tick_mask: None,
        }];
        // SAFETY: the read-only program declares no fields, so no pointer is
        // dereferenced for these rows.
        let evaluated =
            unsafe { evaluate_batch_program(&batches, &read_only, &strides, false) }.unwrap();
        #[cfg(feature = "parallel")]
        {
            // SAFETY: the read-only program still declares no fields.
            let parallel =
                unsafe { evaluate_batch_program(&batches, &read_only, &strides, true) }.unwrap();
            assert_eq!(parallel, evaluated);
        }

        assert_eq!(batched, filtered);
        assert_eq!(batched, evaluated);
        assert_ne!(
            batched[0], batched[32_768],
            "the seed must not restart at the chunk boundary"
        );
    }

    #[test]
    fn random_uses_the_full_entity_seed() {
        let mut seen = HashMap::new();
        let (first, second) = (1..)
            .map(Entity::from_bits)
            .find_map(|entity| {
                let low = entity_random_seed(entity) as u32;
                seen.insert(low, entity)
                    .filter(|&earlier| earlier != entity)
                    .map(|earlier| (earlier, entity))
            })
            .expect("32-bit seed prefixes collide well before 2**32 entities");
        let mut compiler = Compiler::new();
        compiler.emit(Op::Random(0));
        let bytecode = compiler.finalize();
        let draw = |entity: Entity| {
            let mut vm = VM::new();
            // SAFETY: the stack-only program reads no fields.
            unsafe { vm.execute_and_reduce(&bytecode, &[], entity_random_seed(entity)) }
        };
        assert_ne!(draw(first), draw(second));
    }

    #[test]
    fn random_follows_the_entity_not_its_table_row() {
        let cid = ComponentId::new(0);
        let strides = HashMap::from([(cid, size_of::<f64>())]);

        let mut compiler = Compiler::new();
        let field = compiler.add_field(FieldId {
            component_id: cid,
            offset: 0,
            field_type: FieldType::F64,
        });
        compiler.emit(Op::Random(0));
        compiler.emit(Op::StoreField(field));
        let assignment = compiler.finalize();

        let run = |table_id: TableId, first_entity: u64| {
            let mut values = vec![0.0_f64; 8];
            let batches = [TableBatch {
                table_id,
                component_bases: HashMap::from([(cid, values.as_mut_ptr().cast::<u8>())]),
                entities: test_entities(first_entity, values.len()),
                start_row: 0,
                entity_count: values.len(),
                table_entity_count: values.len(),
                tick_mask: None,
            }];
            // SAFETY: the batch describes one live, exclusively held f64 run.
            unsafe { execute_batch_assignment(&batches, &assignment, &strides, false) }.unwrap();
            values
        };

        // The same entities keep their values after moving to another table.
        assert_eq!(
            run(TableId::from_u32(0), 0),
            run(TableId::from_u32(7), 0),
            "random must not depend on the table an entity happens to sit in"
        );
        // Different entities at the same rows get different values.
        assert_ne!(run(TableId::from_u32(0), 0), run(TableId::from_u32(0), 100));
    }

    /// Run one integer program through the batched (tiled-eligible) path and
    /// the tick-filtered (scalar VM) path, and return both results.
    fn both_paths_i64(bytecode: &CompiledBytecode, cid: ComponentId, start: i64) -> (i64, i64) {
        let strides = HashMap::from([(cid, size_of::<i64>())]);
        let run = |mask: Option<Vec<bool>>| {
            let mut value = [start];
            let batches = [TableBatch {
                table_id: TableId::from_u32(0),
                component_bases: HashMap::from([(cid, value.as_mut_ptr().cast::<u8>())]),
                entities: test_entities(0, 1),
                start_row: 0,
                entity_count: 1,
                table_entity_count: 1,
                tick_mask: mask,
            }];
            // SAFETY: the batch describes one live, exclusively held i64.
            unsafe {
                if batches[0].tick_mask.is_some() {
                    execute_filtered_assignment(&batches, bytecode, &strides, false).unwrap();
                } else {
                    execute_batch_assignment(&batches, bytecode, &strides, false).unwrap();
                }
            }
            value[0]
        };
        (run(None), run(Some(vec![true])))
    }

    /// `field = field + (lhs OP rhs)` over one i64 field.
    fn constant_subexpression_program(
        cid: ComponentId,
        lhs: f64,
        rhs: f64,
        op: Op,
    ) -> (CompiledBytecode, ComponentId) {
        let mut compiler = Compiler::new();
        let field = compiler.add_field(FieldId {
            component_id: cid,
            offset: 0,
            field_type: FieldType::I64,
        });
        let lhs = compiler.add_constant(lhs);
        let rhs = compiler.add_constant(rhs);
        compiler.emit(Op::PushField(field));
        compiler.emit(Op::PushConst(lhs));
        compiler.emit(Op::PushConst(rhs));
        compiler.emit(op);
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field));
        (compiler.finalize(), cid)
    }

    #[test]
    fn partially_folded_constants_agree_on_both_paths() {
        // Every surviving constant fits an i64 lane; their sum does not.
        let cid = ComponentId::new(0);
        let k = 2_f64.powi(62);
        let field = RustExpr::Field {
            component_id: cid,
            offset: 0,
            field_type: FieldType::I64,
        };
        let expression = RustExpr::Add(
            Box::new(field),
            Box::new(RustExpr::Add(
                Box::new(RustExpr::Sub(
                    Box::new(RustExpr::Const(2.0 * k)),
                    Box::new(RustExpr::Const(k)),
                )),
                Box::new(RustExpr::Min(
                    Box::new(RustExpr::Const(k)),
                    Box::new(RustExpr::Const(k)),
                )),
            )),
        );
        let bytecode = compile_assignment(cid, 0, FieldType::I64, &expression);
        let (batched, filtered) = both_paths_i64(&bytecode, cid, 1);
        assert_eq!(batched, filtered);
    }

    #[test]
    fn constant_subexpressions_stay_exact_on_both_paths() {
        let cid = ComponentId::new(0);

        // Above 2^53 a constant that degraded to f64 would lose the low bit.
        let (bytecode, _) = constant_subexpression_program(cid, 1.0, 1.0, Op::Min);
        let (batched, filtered) = both_paths_i64(&bytecode, cid, (1_i64 << 60) + 1);
        assert_eq!(batched, filtered);
        assert_eq!(batched, (1_i64 << 60) + 2);

        // Every constant-folding op keeps its result exact the same way.
        for op in [Op::Add, Op::Sub, Op::Mul, Op::Min, Op::Max] {
            let (bytecode, _) = constant_subexpression_program(cid, 6.0, 2.0, op);
            let (batched, filtered) = both_paths_i64(&bytecode, cid, (1_i64 << 60) + 1);
            assert_eq!(batched, filtered, "op={op:?}");
        }
    }

    #[test]
    fn out_of_lane_constant_arithmetic_agrees_on_both_paths() {
        // 200 and 100 each fit a u8 lane but their sum does not.
        let cid = ComponentId::new(0);
        let mut compiler = Compiler::new();
        let field = compiler.add_field(FieldId {
            component_id: cid,
            offset: 0,
            field_type: FieldType::U8,
        });
        let big = compiler.add_constant(200.0);
        let small = compiler.add_constant(100.0);
        compiler.emit(Op::PushField(field));
        compiler.emit(Op::PushConst(big));
        compiler.emit(Op::PushConst(small));
        compiler.emit(Op::Add);
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field));
        let mut bytecode = compiler.finalize();
        assert_eq!(bytecode.bytecode.len(), 6, "unfolded program under test");

        let strides = HashMap::from([(cid, size_of::<u8>())]);
        let run = |bytecode: &CompiledBytecode, mask: Option<Vec<bool>>| {
            let mut value = [250_u8];
            let batches = [TableBatch {
                table_id: TableId::from_u32(0),
                component_bases: HashMap::from([(cid, value.as_mut_ptr())]),
                entities: test_entities(0, 1),
                start_row: 0,
                entity_count: 1,
                table_entity_count: 1,
                tick_mask: mask,
            }];
            // SAFETY: the batch describes one live, exclusively held u8.
            unsafe {
                if batches[0].tick_mask.is_some() {
                    execute_filtered_assignment(&batches, bytecode, &strides, false).unwrap();
                } else {
                    execute_batch_assignment(&batches, bytecode, &strides, false).unwrap();
                }
            }
            value[0]
        };

        // Unfolded, the constant-only Add is declined.
        assert!(
            !supported_program(&bytecode),
            "a constant-only operation must not reach the integer lanes"
        );
        assert_eq!(run(&bytecode, None), run(&bytecode, Some(vec![true])));

        // Folded, 300 fails the u8 range check instead.
        let mut folded = Compiler::new();
        let field = folded.add_field(FieldId {
            component_id: cid,
            offset: 0,
            field_type: FieldType::U8,
        });
        let big = folded.add_constant(200.0);
        let small = folded.add_constant(100.0);
        folded.emit(Op::PushField(field));
        folded.emit(Op::PushConst(big));
        folded.emit(Op::PushConst(small));
        folded.emit(Op::Add);
        folded.emit(Op::Add);
        folded.emit(Op::StoreField(field));
        folded.optimize();
        bytecode = folded.finalize();
        assert_eq!(bytecode.bytecode.len(), 4, "the constant pair folded");
        assert!(!supported_program(&bytecode));
        assert_eq!(run(&bytecode, None), run(&bytecode, Some(vec![true])));
    }

    #[test]
    fn test_compile_assignment_produces_valid_bytecode() {
        let cid = ComponentId::new(0);
        let expr = RustExpr::Add(
            Box::new(RustExpr::Field {
                component_id: cid,
                offset: 0,
                field_type: FieldType::F32,
            }),
            Box::new(RustExpr::Const(3.0)),
        );

        let bytecode = compile_assignment(cid, 0, FieldType::F32, &expr);
        // PushField(0), PushConst(3.0), Add, StoreField(0)
        assert_eq!(bytecode.bytecode.len(), 4);

        let mut value = 7.0_f32;
        let ptr = &mut value as *mut f32 as *mut u8;
        let mut vm = VM::new();
        // SAFETY: the program's only field is this live, exclusively held f32.
        unsafe { vm.execute(&bytecode, &[ptr], 0) };
        assert_eq!(value, 10.0);
    }

    #[test]
    fn offset_validator_accepts_field_within_layout() {
        let mut world = World::new();
        let cid = world.register_component::<Probe>();
        // Probe is 12 bytes; an F32 at offset 8 spans [8, 12) - the last valid slot.
        let bytecode = make_typed_read_bytecode(cid, 8, FieldType::F32);
        assert!(validate_bytecode_offsets(&world, &bytecode).is_ok());
    }

    #[test]
    fn offset_validator_rejects_offset_past_layout() {
        let mut world = World::new();
        let cid = world.register_component::<Probe>();
        // offset 12 + 4-byte F32 = span 16 > 12 -> out of bounds.
        let bytecode = make_typed_read_bytecode(cid, 12, FieldType::F32);
        assert!(matches!(
            validate_bytecode_offsets(&world, &bytecode),
            Err(ViewEngineError::FieldOffsetOutOfBounds { .. })
        ));
    }

    #[test]
    fn offset_validator_rejects_wide_type_overrunning_last_byte() {
        let mut world = World::new();
        let cid = world.register_component::<Probe>();
        // A Bool (1 byte) at the final byte (offset 11) fits (span 12); an F64
        // (8 bytes) at the same offset overruns the 12-byte layout.
        assert!(
            validate_bytecode_offsets(&world, &make_typed_read_bytecode(cid, 11, FieldType::Bool))
                .is_ok()
        );
        assert!(matches!(
            validate_bytecode_offsets(&world, &make_typed_read_bytecode(cid, 11, FieldType::F64)),
            Err(ViewEngineError::FieldOffsetOutOfBounds { .. })
        ));
    }

    #[test]
    fn offset_validator_rejects_overflowing_offset() {
        let mut world = World::new();
        let cid = world.register_component::<Probe>();
        // offset + type_size would overflow usize -> caught before the addition.
        let bytecode = make_typed_read_bytecode(cid, usize::MAX, FieldType::F64);
        assert!(matches!(
            validate_bytecode_offsets(&world, &bytecode),
            Err(ViewEngineError::FieldOffsetOutOfBounds { .. })
        ));
    }

    #[test]
    fn offset_validator_rejects_unregistered_component() {
        let world = World::new();
        let bytecode = make_typed_read_bytecode(ComponentId::new(9999), 0, FieldType::F32);
        assert!(matches!(
            validate_bytecode_offsets(&world, &bytecode),
            Err(ViewEngineError::ComponentNotFound(_))
        ));
    }

    #[test]
    fn component_validator_accepts_declared_component() {
        let cid = ComponentId::new(3);
        let allowed: HashSet<ComponentId> = [cid].into_iter().collect();
        let bytecode = make_typed_read_bytecode(cid, 0, FieldType::F32);
        assert!(validate_bytecode_components(&bytecode, &allowed).is_ok());
    }

    #[test]
    fn component_validator_rejects_undeclared_component() {
        let declared = ComponentId::new(3);
        let forged = ComponentId::new(7);
        let allowed: HashSet<ComponentId> = [declared].into_iter().collect();
        let bytecode = make_typed_read_bytecode(forged, 0, FieldType::F32);
        assert!(matches!(
            validate_bytecode_components(&bytecode, &allowed),
            Err(ViewEngineError::FieldComponentNotDeclared(id)) if id == forged
        ));
    }

    #[test]
    fn component_validator_checks_store_dest_not_just_reads() {
        // An assignment that reads a declared component but writes an undeclared
        // one must be rejected: both PushField and StoreField dests live in
        // field_map, so the validator has to scan every entry.
        let declared = ComponentId::new(1);
        let forged = ComponentId::new(2);
        let allowed: HashSet<ComponentId> = [declared].into_iter().collect();

        let mut compiler = Compiler::new();
        let read_idx = compiler.add_field(FieldId {
            component_id: declared,
            offset: 0,
            field_type: FieldType::F32,
        });
        let write_idx = compiler.add_field(FieldId {
            component_id: forged,
            offset: 0,
            field_type: FieldType::F32,
        });
        compiler.emit(Op::PushField(read_idx));
        compiler.emit(Op::StoreField(write_idx));
        let bytecode = compiler.finalize();

        assert!(matches!(
            validate_bytecode_components(&bytecode, &allowed),
            Err(ViewEngineError::FieldComponentNotDeclared(id)) if id == forged
        ));
    }

    #[test]
    fn component_validator_accepts_empty_field_map() {
        // A constant-only expression references no fields - nothing to reject.
        let allowed: HashSet<ComponentId> = HashSet::new();
        let bytecode = CompiledBytecode {
            bytecode: vec![],
            constants: vec![],
            integers: Vec::new(),
            field_map: vec![],
        };
        assert!(validate_bytecode_components(&bytecode, &allowed).is_ok());
    }

    /// Accept-set for a single component (offsets already vector-expanded).
    fn allowed_fields_for(
        cid: ComponentId,
        pairs: &[(usize, FieldType)],
    ) -> HashMap<ComponentId, HashSet<(usize, FieldType)>> {
        let mut map = HashMap::new();
        map.insert(cid, pairs.iter().copied().collect());
        map
    }

    #[test]
    fn field_type_validator_accepts_real_fields() {
        // Three f32 fields at 0/4/8 (a Vec3's expanded lanes, or three scalars).
        let cid = ComponentId::new(3);
        let allowed = allowed_fields_for(
            cid,
            &[
                (0, FieldType::F32),
                (4, FieldType::F32),
                (8, FieldType::F32),
            ],
        );
        for offset in [0usize, 4, 8] {
            let bytecode = make_typed_read_bytecode(cid, offset, FieldType::F32);
            assert!(validate_bytecode_field_types(&bytecode, &allowed).is_ok());
        }
    }

    #[test]
    fn field_type_validator_rejects_bool_over_f32_field() {
        // The sharp case: a real f32 field read as Bool. At execution that reads a
        // non-0/1 byte as `bool` - an invalid bit pattern and instant UB.
        let cid = ComponentId::new(3);
        let allowed = allowed_fields_for(cid, &[(0, FieldType::F32)]);
        let bytecode = make_typed_read_bytecode(cid, 0, FieldType::Bool);
        assert!(matches!(
            validate_bytecode_field_types(&bytecode, &allowed),
            Err(ViewEngineError::FieldTypeMismatch {
                component_id,
                offset: 0,
                field_type: FieldType::Bool,
            }) if component_id == cid
        ));
    }

    #[test]
    fn field_type_validator_rejects_mid_field_offset() {
        // Offset 2 lands inside the first f32 field ([0,4)); not a field start, so
        // `validate_bytecode_offsets` (bounds-only) would allow it but this rejects it.
        let cid = ComponentId::new(3);
        let allowed = allowed_fields_for(cid, &[(0, FieldType::F32), (4, FieldType::F32)]);
        let bytecode = make_typed_read_bytecode(cid, 2, FieldType::F32);
        assert!(matches!(
            validate_bytecode_field_types(&bytecode, &allowed),
            Err(ViewEngineError::FieldTypeMismatch { offset: 2, .. })
        ));
    }

    #[test]
    fn field_type_validator_rejects_type_pun_wider_than_field() {
        // Reading an f32 field as I64: in-bounds within a 3-f32 layout (passes the
        // offset validator), but not a real field -> type confusion / info leak.
        let cid = ComponentId::new(3);
        let allowed = allowed_fields_for(
            cid,
            &[
                (0, FieldType::F32),
                (4, FieldType::F32),
                (8, FieldType::F32),
            ],
        );
        let bytecode = make_typed_read_bytecode(cid, 0, FieldType::I64);
        assert!(matches!(
            validate_bytecode_field_types(&bytecode, &allowed),
            Err(ViewEngineError::FieldTypeMismatch {
                field_type: FieldType::I64,
                ..
            })
        ));
    }

    #[test]
    fn field_type_validator_rejects_undeclared_component() {
        let declared = ComponentId::new(3);
        let forged = ComponentId::new(7);
        let allowed = allowed_fields_for(declared, &[(0, FieldType::F32)]);
        let bytecode = make_typed_read_bytecode(forged, 0, FieldType::F32);
        assert!(matches!(
            validate_bytecode_field_types(&bytecode, &allowed),
            Err(ViewEngineError::FieldComponentNotDeclared(id)) if id == forged
        ));
    }

    #[test]
    fn field_type_validator_checks_store_dest_not_just_reads() {
        // Read a real f32 field, but write a confused I64 at the same offset. Both
        // FieldIds live in field_map, so the store dest must also be checked.
        let cid = ComponentId::new(1);
        let allowed = allowed_fields_for(cid, &[(0, FieldType::F32)]);
        let mut compiler = Compiler::new();
        let read_idx = compiler.add_field(FieldId {
            component_id: cid,
            offset: 0,
            field_type: FieldType::F32,
        });
        let write_idx = compiler.add_field(FieldId {
            component_id: cid,
            offset: 0,
            field_type: FieldType::I64,
        });
        compiler.emit(Op::PushField(read_idx));
        compiler.emit(Op::StoreField(write_idx));
        let bytecode = compiler.finalize();
        assert!(matches!(
            validate_bytecode_field_types(&bytecode, &allowed),
            Err(ViewEngineError::FieldTypeMismatch {
                field_type: FieldType::I64,
                ..
            })
        ));
    }

    #[test]
    fn field_type_validator_accepts_empty_field_map() {
        let allowed: HashMap<ComponentId, HashSet<(usize, FieldType)>> = HashMap::new();
        let bytecode = CompiledBytecode {
            bytecode: vec![],
            constants: vec![],
            integers: Vec::new(),
            field_map: vec![],
        };
        assert!(validate_bytecode_field_types(&bytecode, &allowed).is_ok());
    }

    #[derive(Component)]
    struct RowA(#[allow(dead_code)] f32);

    #[derive(Component)]
    struct RowSkip;

    fn row_filter(world: &mut World, without: Option<ComponentId>) -> ViewFilter {
        let mut component_ids = HashSet::new();
        component_ids.insert(world.register_component::<RowA>());
        ViewFilter {
            component_ids,
            with_ids: Vec::new(),
            without_ids: without.into_iter().collect(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        }
    }

    #[test]
    fn whole_table_selection_needs_no_row_walk() {
        let mut world = World::new();
        for i in 0..8 {
            world.spawn(RowA(i as f32));
        }
        let filter = row_filter(&mut world, None);
        let ranges = matching_table_row_ranges(&world, &filter);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_row, 0);
        assert_eq!(ranges[0].entity_count, 8);
    }

    #[test]
    fn partially_selected_table_still_reports_exact_runs() {
        let mut world = World::new();
        // Rows 0..6 in one table; the odd ones also carry RowSkip, which the
        // filter excludes, so selection is non-contiguous and the fast path
        // must not fire.
        for i in 0..6 {
            if i % 2 == 0 {
                world.spawn(RowA(i as f32));
            } else {
                world.spawn((RowA(i as f32), RowSkip));
            }
        }
        let skip = world.register_component::<RowSkip>();
        let filter = row_filter(&mut world, Some(skip));
        let ranges = matching_table_row_ranges(&world, &filter);
        let selected: usize = ranges.iter().map(|r| r.entity_count).sum();
        assert_eq!(selected, 3, "three entities lack RowSkip");
        for range in &ranges {
            assert!(range.entity_count > 0);
        }
    }

    #[test]
    fn empty_selection_yields_no_ranges() {
        let mut world = World::new();
        world.spawn(RowSkip);
        let filter = row_filter(&mut world, None);
        assert!(matching_table_row_ranges(&world, &filter).is_empty());
    }
}
