//! Interpreter-neutral row writer for custom `@component` `batch` batches.
//! The interpreter-free half of `src/ecs/custom_batch.rs`: given typed columns
//! ([`ColumnData`]) and a [`ComponentLayout`], it materializes zero-initialized
//! wrapper-sized byte rows with each field written at its `PrimitiveType`-keyed
//! offset. Shared so the pyo3 and RustPython custom-batch adapters cannot drift
//! on byte layout. The interpreter leaves (numpy borrows, PyObject storage
//! rejection, `OwningPtr` insertion) stay in each backend's adapter.

use pybevy_storage::batch_columns::{ColumnDType, ColumnData};

use crate::component_layout::{ComponentLayout, PrimitiveType};

/// A borrowed typed column feeding one field of every row. `Vec3`/`Vec2` are
/// flat `f32` slices with stride 3/2 per entity.
pub enum FieldColumn<'a> {
    F32(&'a [f32]),
    F64(&'a [f64]),
    I32(&'a [i32]),
    I64(&'a [i64]),
    U32(&'a [u32]),
    U64(&'a [u64]),
    Bool(&'a [u8]),
    /// Flat slice of f32 from an (N, 3) array: stride 3 per entity.
    Vec3(&'a [f32]),
    /// Flat slice of f32 from an (N, 2) array: stride 2 per entity.
    Vec2(&'a [f32]),
}

impl FieldColumn<'_> {
    /// Write the value at `index` into `buffer` at the given byte offset. Moved
    /// verbatim from the pyo3 `FieldSlice::write_to_buffer` (same little-endian
    /// encoding and Vec3/Vec2 stride lanes).
    #[inline(always)]
    pub fn write_to_buffer(&self, index: usize, buffer: &mut [u8], offset: usize) {
        match self {
            FieldColumn::F32(s) => {
                buffer[offset..offset + 4].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldColumn::F64(s) => {
                buffer[offset..offset + 8].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldColumn::I32(s) => {
                buffer[offset..offset + 4].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldColumn::I64(s) => {
                buffer[offset..offset + 8].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldColumn::U32(s) => {
                buffer[offset..offset + 4].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldColumn::U64(s) => {
                buffer[offset..offset + 8].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldColumn::Bool(s) => {
                buffer[offset] = s[index];
            }
            FieldColumn::Vec3(s) => {
                let base = index * 3;
                buffer[offset..offset + 4].copy_from_slice(&s[base].to_le_bytes());
                buffer[offset + 4..offset + 8].copy_from_slice(&s[base + 1].to_le_bytes());
                buffer[offset + 8..offset + 12].copy_from_slice(&s[base + 2].to_le_bytes());
            }
            FieldColumn::Vec2(s) => {
                let base = index * 2;
                buffer[offset..offset + 4].copy_from_slice(&s[base].to_le_bytes());
                buffer[offset + 4..offset + 8].copy_from_slice(&s[base + 1].to_le_bytes());
            }
        }
    }
}

/// The extraction target for a `PrimitiveType`-keyed field: `(column dtype,
/// columns per entity)`. `Vec3 -> (F32, 3)`, `Vec2 -> (F32, 2)`,
/// `Bool -> (Bool, 1)`, everything else -> `(matching dtype, 1)`.
pub fn column_dtype_for(field_type: PrimitiveType) -> (ColumnDType, usize) {
    match field_type {
        PrimitiveType::F32 => (ColumnDType::F32, 1),
        PrimitiveType::F64 => (ColumnDType::F64, 1),
        PrimitiveType::I32 => (ColumnDType::I32, 1),
        PrimitiveType::I64 => (ColumnDType::I64, 1),
        PrimitiveType::U32 => (ColumnDType::U32, 1),
        PrimitiveType::U64 => (ColumnDType::U64, 1),
        PrimitiveType::Bool => (ColumnDType::Bool, 1),
        PrimitiveType::Vec3 => (ColumnDType::F32, 3),
        PrimitiveType::Vec2 => (ColumnDType::F32, 2),
    }
}

/// View an owned [`ColumnData`] as the [`FieldColumn`] for `field_type`. Returns
/// `None` when the payload dtype does not match the field (an adapter bug: the
/// column was extracted at the wrong dtype).
pub fn field_column_for(field_type: PrimitiveType, data: &ColumnData) -> Option<FieldColumn<'_>> {
    match (field_type, data) {
        (PrimitiveType::F32, ColumnData::F32(v)) => Some(FieldColumn::F32(v)),
        (PrimitiveType::F64, ColumnData::F64(v)) => Some(FieldColumn::F64(v)),
        (PrimitiveType::I32, ColumnData::I32(v)) => Some(FieldColumn::I32(v)),
        (PrimitiveType::I64, ColumnData::I64(v)) => Some(FieldColumn::I64(v)),
        (PrimitiveType::U32, ColumnData::U32(v)) => Some(FieldColumn::U32(v)),
        (PrimitiveType::U64, ColumnData::U64(v)) => Some(FieldColumn::U64(v)),
        (PrimitiveType::Bool, ColumnData::Bool(v)) => Some(FieldColumn::Bool(v)),
        (PrimitiveType::Vec3, ColumnData::F32(v)) => Some(FieldColumn::Vec3(v)),
        (PrimitiveType::Vec2, ColumnData::F32(v)) => Some(FieldColumn::Vec2(v)),
        _ => None,
    }
}

/// Materialize `count` wrapper-sized, zero-initialized rows and write every
/// provided `(field index into layout.fields, column)` pair at the field's
/// `PrimitiveType`-keyed offset. Unspecified fields stay zero bytes (pyo3
/// semantics: partial `batch` leaves the rest of the wrapper zeroed).
pub fn build_wrapper_rows(
    layout: &ComponentLayout,
    columns: &[(usize, FieldColumn<'_>)],
    count: usize,
) -> Vec<Vec<u8>> {
    let mut rows = Vec::with_capacity(count);
    for index in 0..count {
        let mut buffer = vec![0u8; layout.wrapper_size.size_bytes()];
        for (field_idx, column) in columns {
            column.write_to_buffer(index, &mut buffer, layout.fields[*field_idx].offset);
        }
        rows.push(buffer);
    }
    rows
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::{
        component_layout::{ComponentLayout, FieldInfo, PrimitiveType},
        component_wrapper::WrapperSize,
    };

    fn f32_le(bytes: &[u8]) -> f32 {
        f32::from_le_bytes(bytes.try_into().unwrap())
    }

    #[test]
    fn write_scalar_and_composite_lanes() {
        let f64s = [1.5f64, 2.5];
        let vecs = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut buf = vec![0u8; 24];
        FieldColumn::F64(&f64s).write_to_buffer(1, &mut buf, 0);
        assert_eq!(f64::from_le_bytes(buf[0..8].try_into().unwrap()), 2.5);

        let mut buf = vec![0u8; 24];
        FieldColumn::Vec3(&vecs).write_to_buffer(1, &mut buf, 8);
        assert_eq!(f32_le(&buf[8..12]), 4.0);
        assert_eq!(f32_le(&buf[12..16]), 5.0);
        assert_eq!(f32_le(&buf[16..20]), 6.0);
    }

    #[test]
    fn bool_and_i64_bytes() {
        let bools = [1u8, 0];
        let ints = [7i64, -3];
        let mut buf = vec![0u8; 16];
        FieldColumn::Bool(&bools).write_to_buffer(0, &mut buf, 8);
        assert_eq!(buf[8], 1);
        FieldColumn::I64(&ints).write_to_buffer(1, &mut buf, 0);
        assert_eq!(i64::from_le_bytes(buf[0..8].try_into().unwrap()), -3);
    }

    #[test]
    fn column_dtype_mapping() {
        assert_eq!(column_dtype_for(PrimitiveType::Vec3), (ColumnDType::F32, 3));
        assert_eq!(column_dtype_for(PrimitiveType::Vec2), (ColumnDType::F32, 2));
        assert_eq!(column_dtype_for(PrimitiveType::I64), (ColumnDType::I64, 1));
        assert_eq!(
            column_dtype_for(PrimitiveType::Bool),
            (ColumnDType::Bool, 1)
        );
    }

    #[test]
    fn field_column_dtype_guard() {
        let data = ColumnData::I64(vec![1, 2]);
        assert!(field_column_for(PrimitiveType::I64, &data).is_some());
        assert!(field_column_for(PrimitiveType::F32, &data).is_none());
        let vecs = ColumnData::F32(vec![1.0, 2.0, 3.0]);
        assert!(matches!(
            field_column_for(PrimitiveType::Vec3, &vecs),
            Some(FieldColumn::Vec3(_))
        ));
    }

    #[test]
    fn write_all_remaining_scalar_lanes() {
        let f32s = [1.5f32, -2.5];
        let i32s = [-3i32, 4];
        let u32s = [5u32, 6];
        let u64s = [7u64, 8];

        let mut buf = vec![0u8; 8];
        FieldColumn::F32(&f32s).write_to_buffer(1, &mut buf, 0);
        assert_eq!(f32_le(&buf[0..4]), -2.5);
        FieldColumn::I32(&i32s).write_to_buffer(1, &mut buf, 4);
        assert_eq!(i32::from_le_bytes(buf[4..8].try_into().unwrap()), 4);

        let mut buf = vec![0u8; 16];
        FieldColumn::U32(&u32s).write_to_buffer(1, &mut buf, 0);
        assert_eq!(u32::from_le_bytes(buf[0..4].try_into().unwrap()), 6);
        FieldColumn::U64(&u64s).write_to_buffer(1, &mut buf, 8);
        assert_eq!(u64::from_le_bytes(buf[8..16].try_into().unwrap()), 8);
    }

    #[test]
    fn write_vec2_lane() {
        let vecs = [1.0f32, 2.0, 3.0, 4.0];
        let mut buf = vec![0u8; 12];
        FieldColumn::Vec2(&vecs).write_to_buffer(1, &mut buf, 4);
        assert_eq!(f32_le(&buf[4..8]), 3.0);
        assert_eq!(f32_le(&buf[8..12]), 4.0);
    }

    #[test]
    fn column_dtype_covers_every_scalar_kind() {
        assert_eq!(column_dtype_for(PrimitiveType::F32), (ColumnDType::F32, 1));
        assert_eq!(column_dtype_for(PrimitiveType::F64), (ColumnDType::F64, 1));
        assert_eq!(column_dtype_for(PrimitiveType::I32), (ColumnDType::I32, 1));
        assert_eq!(column_dtype_for(PrimitiveType::U32), (ColumnDType::U32, 1));
        assert_eq!(column_dtype_for(PrimitiveType::U64), (ColumnDType::U64, 1));
    }

    #[test]
    fn field_column_views_every_matching_payload() {
        assert!(matches!(
            field_column_for(PrimitiveType::F32, &ColumnData::F32(vec![1.0])),
            Some(FieldColumn::F32(_))
        ));
        assert!(matches!(
            field_column_for(PrimitiveType::F64, &ColumnData::F64(vec![1.0])),
            Some(FieldColumn::F64(_))
        ));
        assert!(matches!(
            field_column_for(PrimitiveType::I32, &ColumnData::I32(vec![1])),
            Some(FieldColumn::I32(_))
        ));
        assert!(matches!(
            field_column_for(PrimitiveType::U32, &ColumnData::U32(vec![1])),
            Some(FieldColumn::U32(_))
        ));
        assert!(matches!(
            field_column_for(PrimitiveType::U64, &ColumnData::U64(vec![1])),
            Some(FieldColumn::U64(_))
        ));
        assert!(matches!(
            field_column_for(PrimitiveType::Bool, &ColumnData::Bool(vec![1])),
            Some(FieldColumn::Bool(_))
        ));
        assert!(matches!(
            field_column_for(PrimitiveType::Vec2, &ColumnData::F32(vec![1.0, 2.0])),
            Some(FieldColumn::Vec2(_))
        ));
    }

    #[test]
    fn build_rows_writes_offsets_and_zero_fills() {
        // Two fields: `a` (F64 at offset 0), `b` (I64 at offset 8); wrapper 16.
        let layout = ComponentLayout {
            py_type_ptr: std::ptr::null(),
            name: "T".to_string(),
            fields: vec![
                FieldInfo {
                    name: "a".into(),
                    offset: 0,
                    field_type: PrimitiveType::F64,
                },
                FieldInfo {
                    name: "b".into(),
                    offset: 8,
                    field_type: PrimitiveType::I64,
                },
            ],
            data_size: 16,
            wrapper_size: crate::component_wrapper::WrapperSize::W16,
        };
        let a = [1.0f64, 2.0];
        // Only field `a` provided; `b` stays zero bytes.
        let rows = build_wrapper_rows(&layout, &[(0, FieldColumn::F64(&a))], 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(f64::from_le_bytes(rows[1][0..8].try_into().unwrap()), 2.0);
        assert_eq!(i64::from_le_bytes(rows[1][8..16].try_into().unwrap()), 0);
    }

    #[test]
    fn build_rows_writes_every_provided_field_without_clobbering() {
        // Fields: `a` (F64 at 0), `b` (I64 at 8), `v` (Vec2 at 16); wrapper 32.
        let layout = ComponentLayout {
            py_type_ptr: std::ptr::null(),
            name: "T".to_string(),
            fields: vec![
                FieldInfo {
                    name: "a".into(),
                    offset: 0,
                    field_type: PrimitiveType::F64,
                },
                FieldInfo {
                    name: "b".into(),
                    offset: 8,
                    field_type: PrimitiveType::I64,
                },
                FieldInfo {
                    name: "v".into(),
                    offset: 16,
                    field_type: PrimitiveType::Vec2,
                },
            ],
            data_size: 24,
            wrapper_size: WrapperSize::W32,
        };
        let a = [1.5f64, -2.5];
        let b = [7i64, -8];
        let v = [1.0f32, 2.0, 3.0, 4.0];

        let rows = build_wrapper_rows(
            &layout,
            &[
                (0, FieldColumn::F64(&a)),
                (1, FieldColumn::I64(&b)),
                (2, FieldColumn::Vec2(&v)),
            ],
            2,
        );

        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                f64::from_le_bytes(rows[0][0..8].try_into().unwrap()),
                i64::from_le_bytes(rows[0][8..16].try_into().unwrap()),
                f32_le(&rows[0][16..20]),
                f32_le(&rows[0][20..24])
            ),
            (1.5f64, 7i64, 1.0f32, 2.0)
        );
        assert_eq!(
            (
                f64::from_le_bytes(rows[1][0..8].try_into().unwrap()),
                i64::from_le_bytes(rows[1][8..16].try_into().unwrap()),
                f32_le(&rows[1][16..20]),
                f32_le(&rows[1][20..24])
            ),
            (-2.5f64, -8i64, 3.0f32, 4.0)
        );
        assert_eq!(rows[0].len(), 32);
        assert_eq!(&rows[0][24..], &[0u8; 8]);
        assert_ne!(rows[0], rows[1]);
    }

    #[test]
    fn build_rows_row_length_follows_the_wrapper_size() {
        let make_layout = |wrapper_size: WrapperSize| ComponentLayout {
            py_type_ptr: std::ptr::null(),
            name: "T".to_string(),
            fields: vec![FieldInfo {
                name: "flag".into(),
                offset: 0,
                field_type: PrimitiveType::Bool,
            }],
            data_size: 1,
            wrapper_size,
        };
        let flags = [1u8];

        let small = build_wrapper_rows(
            &make_layout(WrapperSize::W8),
            &[(0, FieldColumn::Bool(&flags))],
            1,
        );
        let large = build_wrapper_rows(
            &make_layout(WrapperSize::W64),
            &[(0, FieldColumn::Bool(&flags))],
            1,
        );

        assert_eq!(small.len(), 1);
        assert_eq!(large.len(), 1);
        assert_eq!(small[0].len(), 8);
        assert_eq!(large[0].len(), 64);
        // Bool occupies exactly one byte; every padding byte stays zero.
        assert_eq!(small[0][0], 1);
        assert_eq!(&small[0][1..], &[0u8; 7]);
        assert_eq!(large[0][0], 1);
        assert_eq!(&large[0][1..], &[0u8; 63]);
    }
}
