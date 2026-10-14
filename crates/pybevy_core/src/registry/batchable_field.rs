//! Trait for component fields that can be batch-spawned from numpy arrays.
//!
//! `BatchableField` provides type-level metadata and conversion logic
//! for each field type supported in batch spawning. The macro-generated
//! batch insert functions use this trait to resolve field properties
//! at compile time via type inference.

use bevy::{
    color::{Color, Srgba},
    math::{Quat, Vec2, Vec3, Vec4},
};
use pybevy_storage::{
    FieldType,
    batch_columns::IntegerColumn,
    domain::{DomainValues, DomainViolation, FieldDomain},
};

/// A component field type that can be populated from numpy float32 data.
///
/// All batch fields use f32 numpy arrays (matching the TransformBatch pattern).
/// Multi-element types (Vec3, Quat) use 2D arrays with the appropriate column count.
pub trait BatchableField: Sized + Copy {
    /// Number of f32 elements per value (1 for scalar, 3 for Vec3, 4 for Quat)
    const ELEMENT_COUNT: usize;

    /// Numpy dtype string for the array
    const NUMPY_DTYPE: &'static str;

    /// Number of columns in the numpy array (same as ELEMENT_COUNT)
    const NUMPY_COLUMNS: usize;

    /// View API field type. Only meaningful for types used in `view_fields`.
    const VIEW_FIELD_TYPE: FieldType;

    /// Integer type whose range and integrality `batch` inputs are checked against.
    const INTEGER_COLUMN: Option<IntegerColumn> = None;

    /// Construct a value from a contiguous f32 slice at the given entity index.
    ///
    /// For scalars: `data[index]`
    /// For Vec3: `data[index * 3 .. index * 3 + 3]`
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self;
}

impl BatchableField for f32 {
    const ELEMENT_COUNT: usize = 1;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 1;
    const VIEW_FIELD_TYPE: FieldType = FieldType::F32;

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        data[index]
    }
}

impl BatchableField for bool {
    const ELEMENT_COUNT: usize = 1;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 1;
    const VIEW_FIELD_TYPE: FieldType = FieldType::Bool;

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        data[index] != 0.0
    }
}

impl BatchableField for i32 {
    const ELEMENT_COUNT: usize = 1;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 1;
    const VIEW_FIELD_TYPE: FieldType = FieldType::I32;
    const INTEGER_COLUMN: Option<IntegerColumn> = Some(IntegerColumn::I32);

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        data[index] as i32
    }
}

impl BatchableField for u32 {
    const ELEMENT_COUNT: usize = 1;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 1;
    const VIEW_FIELD_TYPE: FieldType = FieldType::U32;
    const INTEGER_COLUMN: Option<IntegerColumn> = Some(IntegerColumn::U32);

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        data[index] as u32
    }
}

impl BatchableField for Color {
    const ELEMENT_COUNT: usize = 4;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 4;
    const VIEW_FIELD_TYPE: FieldType = FieldType::F32;

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        let idx = index * 4;
        Color::Srgba(Srgba::new(
            data[idx],
            data[idx + 1],
            data[idx + 2],
            data[idx + 3],
        ))
    }
}

impl BatchableField for Vec2 {
    const ELEMENT_COUNT: usize = 2;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 2;
    const VIEW_FIELD_TYPE: FieldType = FieldType::Vec2;

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        let idx = index * 2;
        Vec2::new(data[idx], data[idx + 1])
    }
}

impl BatchableField for Vec3 {
    const ELEMENT_COUNT: usize = 3;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 3;
    const VIEW_FIELD_TYPE: FieldType = FieldType::Vec3;

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        let idx = index * 3;
        Vec3::new(data[idx], data[idx + 1], data[idx + 2])
    }
}

impl BatchableField for Vec4 {
    const ELEMENT_COUNT: usize = 4;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 4;
    const VIEW_FIELD_TYPE: FieldType = FieldType::F32;

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        let idx = index * 4;
        Vec4::new(data[idx], data[idx + 1], data[idx + 2], data[idx + 3])
    }
}

impl BatchableField for Quat {
    const ELEMENT_COUNT: usize = 4;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 4;
    const VIEW_FIELD_TYPE: FieldType = FieldType::Vec4;

    #[inline(always)]
    fn from_numpy_f32_slice(data: &[f32], index: usize) -> Self {
        let idx = index * 4;
        Quat::from_xyzw(data[idx], data[idx + 1], data[idx + 2], data[idx + 3])
    }
}

/// Metadata for a single batchable field, used by ComponentBatchMeta.
pub struct BatchFieldMeta {
    pub name: &'static str,
    pub numpy_columns: usize,
    pub numpy_dtype: &'static str,
    /// Python-facing owner type named in domain violations.
    pub owner: &'static str,
    pub domains: &'static [FieldDomain],
    /// Whether View writes check `domains`.
    pub view_checked: bool,
}

impl BatchFieldMeta {
    pub const fn with_domains(
        mut self,
        owner: &'static str,
        domains: &'static [FieldDomain],
        view_checked: bool,
    ) -> Self {
        self.owner = owner;
        self.domains = domains;
        self.view_checked = view_checked;
        self
    }
}

/// Type-inference helper: compiler resolves T from the `&field` reference.
pub fn batch_field_meta_for<T: BatchableField>(_field: &T, name: &'static str) -> BatchFieldMeta {
    BatchFieldMeta {
        name,
        numpy_columns: T::NUMPY_COLUMNS,
        numpy_dtype: T::NUMPY_DTYPE,
        owner: "",
        domains: &[],
        view_checked: false,
    }
}

/// Check a normalized batch column row by row, as the field stores each value,
/// so an integer field sees the truncated value.
pub fn check_batch_field<T: BatchableField + DomainValues>(
    _field: &T,
    owner: &'static str,
    name: &'static str,
    data: &[f32],
    domains: &'static [FieldDomain],
) -> Result<(), DomainViolation> {
    for row in 0..data.len() / T::NUMPY_COLUMNS {
        T::from_numpy_f32_slice(data, row)
            .check_domains(owner, name, domains)
            .map_err(|violation| violation.at_row(row))?;
    }
    Ok(())
}

/// View API metadata helper: returns the `FieldType` for a field type.
///
/// Uses type inference to resolve T from a `&field` reference, then returns
/// the `FieldType` for View API `FieldOffset` construction.
pub fn field_type_of<T: BatchableField>(_field: &T) -> FieldType {
    T::VIEW_FIELD_TYPE
}

/// Batch input check helper: `Some` for integer fields, resolved from `&field`.
pub fn batch_integer_column_of<T: BatchableField>(_field: &T) -> Option<IntegerColumn> {
    T::INTEGER_COLUMN
}

/// Assignment helper: compiler resolves T from `&mut field`.
#[inline(always)]
pub fn set_field_from_numpy<T: BatchableField>(field: &mut T, data: &[f32], index: usize) {
    *field = T::from_numpy_f32_slice(data, index);
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    const FINITE: &[FieldDomain] = &[FieldDomain::Finite];
    const POSITIVE: &[FieldDomain] = &[FieldDomain::Finite, FieldDomain::Positive];

    #[test]
    fn view_field_types_for_declared_view_field_channels() {
        // These scalar channels are selected by existing component view fields.
        assert_eq!(f32::VIEW_FIELD_TYPE, FieldType::F32);
        assert_eq!(bool::VIEW_FIELD_TYPE, FieldType::Bool);
        assert_eq!(i32::VIEW_FIELD_TYPE, FieldType::I32);
        assert_eq!(u32::VIEW_FIELD_TYPE, FieldType::U32);
        assert_eq!(field_type_of::<i32>(&0), FieldType::I32);
        assert_eq!(field_type_of::<u32>(&0), FieldType::U32);
    }

    #[test]
    fn integer_fields_declare_their_checked_column() {
        assert_eq!(batch_integer_column_of(&0u32), Some(IntegerColumn::U32));
        assert_eq!(batch_integer_column_of(&0i32), Some(IntegerColumn::I32));
        assert_eq!(batch_integer_column_of(&0.0f32), None);
        assert_eq!(batch_integer_column_of(&false), None);
    }

    #[test]
    fn bool_from_numpy_true() {
        let data = [1.0f32, 0.5, 0.01, -1.0, f32::NAN];
        for index in 0..data.len() {
            assert!(bool::from_numpy_f32_slice(&data, index));
        }
    }

    #[test]
    fn bool_from_numpy_false() {
        let data = [0.0f32, -0.0];
        assert!(!bool::from_numpy_f32_slice(&data, 0));
        assert!(!bool::from_numpy_f32_slice(&data, 1));
    }

    #[test]
    fn bool_view_metadata() {
        assert_eq!(bool::VIEW_FIELD_TYPE, FieldType::Bool);
        assert_eq!(bool::NUMPY_COLUMNS, 1);
    }

    #[test]
    fn color_from_numpy_srgba() {
        let data = [0.1f32, 0.2, 0.3, 1.0, 0.5, 0.6, 0.7, 0.8];
        let c0 = Color::from_numpy_f32_slice(&data, 0);
        let c1 = Color::from_numpy_f32_slice(&data, 1);

        match c0 {
            Color::Srgba(srgba) => {
                assert!((srgba.red - 0.1).abs() < 1e-6);
                assert!((srgba.green - 0.2).abs() < 1e-6);
                assert!((srgba.blue - 0.3).abs() < 1e-6);
                assert!((srgba.alpha - 1.0).abs() < 1e-6);
            }
            _ => panic!("Expected Srgba variant"),
        }

        match c1 {
            Color::Srgba(srgba) => {
                assert!((srgba.red - 0.5).abs() < 1e-6);
                assert!((srgba.green - 0.6).abs() < 1e-6);
                assert!((srgba.blue - 0.7).abs() < 1e-6);
                assert!((srgba.alpha - 0.8).abs() < 1e-6);
            }
            _ => panic!("Expected Srgba variant"),
        }
    }

    #[test]
    fn color_batch_metadata() {
        assert_eq!(Color::NUMPY_COLUMNS, 4);
        assert_eq!(Color::ELEMENT_COUNT, 4);
        assert_eq!(Color::NUMPY_DTYPE, "float32");
    }

    #[test]
    fn field_offset_view_meta_f32() {
        let val: f32 = 1.0;
        assert_eq!(field_type_of(&val), FieldType::F32);
    }

    #[test]
    fn field_offset_view_meta_bool() {
        let val: bool = true;
        assert_eq!(field_type_of(&val), FieldType::Bool);
    }

    #[test]
    fn u32_from_numpy() {
        let data = [7.9f32, 0.0, 255.5];
        assert_eq!(u32::from_numpy_f32_slice(&data, 0), 7);
        assert_eq!(u32::from_numpy_f32_slice(&data, 1), 0);
        assert_eq!(u32::from_numpy_f32_slice(&data, 2), 255);
    }

    #[test]
    fn i32_from_numpy() {
        let data = [-7.9f32, 0.0, 255.5];
        assert_eq!(i32::from_numpy_f32_slice(&data, 0), -7);
        assert_eq!(i32::from_numpy_f32_slice(&data, 1), 0);
        assert_eq!(i32::from_numpy_f32_slice(&data, 2), 255);
    }

    #[test]
    fn i32_metadata() {
        assert_eq!(i32::ELEMENT_COUNT, 1);
        assert_eq!(i32::NUMPY_COLUMNS, 1);
        assert_eq!(i32::VIEW_FIELD_TYPE, FieldType::I32);
    }

    #[test]
    fn u32_metadata() {
        assert_eq!(u32::ELEMENT_COUNT, 1);
        assert_eq!(u32::NUMPY_COLUMNS, 1);
        assert_eq!(u32::VIEW_FIELD_TYPE, FieldType::U32);
    }

    #[test]
    fn vec2_from_numpy() {
        let data = [1.0f32, 2.0, 3.0, 4.0];
        let v0 = Vec2::from_numpy_f32_slice(&data, 0);
        let v1 = Vec2::from_numpy_f32_slice(&data, 1);
        assert_eq!(v0, Vec2::new(1.0, 2.0));
        assert_eq!(v1, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn vec2_metadata() {
        assert_eq!(Vec2::ELEMENT_COUNT, 2);
        assert_eq!(Vec2::NUMPY_COLUMNS, 2);
        assert_eq!(Vec2::VIEW_FIELD_TYPE, FieldType::Vec2);
    }

    #[test]
    fn vec3_from_numpy() {
        let data = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let v0 = Vec3::from_numpy_f32_slice(&data, 0);
        let v1 = Vec3::from_numpy_f32_slice(&data, 1);
        assert_eq!(v0, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(v1, Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn vec3_metadata() {
        assert_eq!(Vec3::ELEMENT_COUNT, 3);
        assert_eq!(Vec3::NUMPY_COLUMNS, 3);
        assert_eq!(Vec3::VIEW_FIELD_TYPE, FieldType::Vec3);
    }

    #[test]
    fn vec4_from_numpy() {
        let data = [1.0f32, 2.0, 3.0, 4.0];
        let v = Vec4::from_numpy_f32_slice(&data, 0);
        assert_eq!(v, Vec4::new(1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn vec4_metadata() {
        assert_eq!(Vec4::ELEMENT_COUNT, 4);
        assert_eq!(Vec4::NUMPY_COLUMNS, 4);
    }

    #[test]
    fn quat_view_metadata_preserves_all_four_lanes() {
        assert_eq!(Quat::VIEW_FIELD_TYPE, FieldType::Vec4);
    }

    #[test]
    fn quat_from_numpy() {
        let data = [0.0f32, 0.0, 0.0, 1.0];
        let q = Quat::from_numpy_f32_slice(&data, 0);
        assert_eq!(q, Quat::from_xyzw(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn quat_metadata() {
        assert_eq!(Quat::ELEMENT_COUNT, 4);
        assert_eq!(Quat::NUMPY_COLUMNS, 4);
    }

    #[test]
    fn f32_from_numpy() {
        let data = [1.5f32, 2.71];
        assert!((f32::from_numpy_f32_slice(&data, 0) - 1.5).abs() < 1e-6);
        assert!((f32::from_numpy_f32_slice(&data, 1) - 2.71).abs() < 1e-6);
    }

    #[test]
    fn set_field_helper() {
        let mut val: f32 = 0.0;
        let data = [42.0f32];
        set_field_from_numpy(&mut val, &data, 0);
        assert_eq!(val, 42.0);
    }

    #[test]
    fn set_field_helper_vec3() {
        let mut val = Vec3::ZERO;
        let data = [1.0f32, 2.0, 3.0];
        set_field_from_numpy(&mut val, &data, 0);
        assert_eq!(val, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn batch_field_meta_for_f32() {
        let val: f32 = 0.0;
        let meta = batch_field_meta_for(&val, "speed");
        assert_eq!(meta.name, "speed");
        assert_eq!(meta.numpy_columns, 1);
        assert_eq!(meta.numpy_dtype, "float32");
        assert!(meta.domains.is_empty());
        assert!(!meta.view_checked);
    }

    #[test]
    fn batch_field_meta_for_vec3() {
        let val = Vec3::ZERO;
        let meta = batch_field_meta_for(&val, "position");
        assert_eq!(meta.name, "position");
        assert_eq!(meta.numpy_columns, 3);
    }

    #[test]
    fn batch_field_meta_carries_declared_domains() {
        let meta =
            batch_field_meta_for(&Vec3::ZERO, "position").with_domains("Transform", FINITE, false);
        assert_eq!(
            (meta.owner, meta.domains, meta.view_checked),
            ("Transform", FINITE, false)
        );
    }

    #[test]
    fn batch_columns_are_checked_per_row_and_component() {
        let rows = [1.0f32, 2.0, 3.0, 4.0, f32::NAN, 6.0];
        assert_eq!(
            check_batch_field(&Vec3::ZERO, "Transform", "translation", &rows, FINITE)
                .map_err(|violation| violation.to_string()),
            Err("Transform.translation.y must be finite (got nan at row 1)".into())
        );
        assert!(
            check_batch_field(&Vec3::ZERO, "Transform", "translation", &rows[..3], FINITE).is_ok()
        );
    }

    #[test]
    fn integer_batch_columns_check_the_truncated_value() {
        assert_eq!(
            check_batch_field(
                &0_u32,
                "Bloom",
                "max_mip_dimension",
                &[512.0, 0.5],
                POSITIVE
            )
            .map_err(|violation| violation.to_string()),
            Err("Bloom.max_mip_dimension must be greater than zero (got 0 at row 1)".into())
        );
    }
}
