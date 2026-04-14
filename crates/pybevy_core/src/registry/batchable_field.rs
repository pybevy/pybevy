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
///
/// Usage in macro-generated code:
/// ```ignore
/// let default = PointLight::default();
/// batch_field_meta_for(&default.intensity, "intensity")
/// // T = f32 → columns = 1, dtype = "float32"
/// ```
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
///
/// Usage in macro-generated code:
/// ```ignore
/// let default = PointLight::default();
/// let field_type = field_type_of(&default.shadows_enabled);
/// // T = bool → FieldType::Bool
/// ```
pub fn field_type_of<T: BatchableField>(_field: &T) -> FieldType {
    T::VIEW_FIELD_TYPE
}

/// Assignment helper: compiler resolves T from `&mut field`.
///
/// Usage in macro-generated insert functions:
/// ```ignore
/// set_field_from_numpy(&mut component.intensity, data, i);
/// // T = f32 → component.intensity = data[i]
/// ```
#[inline(always)]
pub fn set_field_from_numpy<T: BatchableField>(field: &mut T, data: &[f32], index: usize) {
    *field = T::from_numpy_f32_slice(data, index);
}
