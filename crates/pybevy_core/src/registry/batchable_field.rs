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
use pybevy_storage::FieldType;

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
        data[index] > 0.0
    }
}

impl BatchableField for u32 {
    const ELEMENT_COUNT: usize = 1;
    const NUMPY_DTYPE: &'static str = "float32";
    const NUMPY_COLUMNS: usize = 1;
    const VIEW_FIELD_TYPE: FieldType = FieldType::U32;

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
    const VIEW_FIELD_TYPE: FieldType = FieldType::F32;

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
}

/// Type-inference helper: compiler resolves T from the `&field` reference.
pub fn batch_field_meta_for<T: BatchableField>(_field: &T, name: &'static str) -> BatchFieldMeta {
    BatchFieldMeta {
        name,
        numpy_columns: T::NUMPY_COLUMNS,
        numpy_dtype: T::NUMPY_DTYPE,
    }
}

/// View API metadata helper: returns the `FieldType` for a field type.
///
/// Uses type inference to resolve T from a `&field` reference, then returns
/// the `FieldType` for View API `FieldOffset` construction.
pub fn field_type_of<T: BatchableField>(_field: &T) -> FieldType {
    T::VIEW_FIELD_TYPE
}

/// Assignment helper: compiler resolves T from `&mut field`.
#[inline(always)]
pub fn set_field_from_numpy<T: BatchableField>(field: &mut T, data: &[f32], index: usize) {
    *field = T::from_numpy_f32_slice(data, index);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_from_numpy_true() {
        let data = [1.0f32, 0.5, 0.01];
        assert!(bool::from_numpy_f32_slice(&data, 0));
        assert!(bool::from_numpy_f32_slice(&data, 1));
        assert!(bool::from_numpy_f32_slice(&data, 2));
    }

    #[test]
    fn bool_from_numpy_false() {
        let data = [0.0f32, -1.0];
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
        let data = [3.14f32, 2.71];
        assert!((f32::from_numpy_f32_slice(&data, 0) - 3.14).abs() < 1e-6);
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
    }

    #[test]
    fn batch_field_meta_for_vec3() {
        let val = Vec3::ZERO;
        let meta = batch_field_meta_for(&val, "position");
        assert_eq!(meta.name, "position");
        assert_eq!(meta.numpy_columns, 3);
    }
}
