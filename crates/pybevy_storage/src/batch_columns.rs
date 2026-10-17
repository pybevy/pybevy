//! Interpreter-neutral `batch` batch-column validation. Shared by the pyo3
//! component macro and the RustPython adapter so shape rules and error strings
//! cannot drift (the "validation, IR, registries, and error definitions" the
//! Backend Unification rules keep neutral). The data work (numpy
//! `reshape`/`astype` on pyo3; `astype`/`to_scalars` on RP2) stays in the
//! adapters; this module owns shape/count validation and View value-domain checks.

use std::fmt;

use crate::{
    domain::{DomainValue, DomainViolation, FieldDomain, check},
    float_repr::float_repr,
};

/// Target dtype of one `batch` column. Covers every
/// `pybevy_core::component_layout::PrimitiveType` payload (Vec3/Vec2 lower to
/// `F32` with `cols = 3/2`) plus the native f32 contract and the Visibility bool
/// column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnDType {
    F32,
    F64,
    I32,
    I64,
    U32,
    U64,
    Bool,
}

/// Owned, contiguous, row-major column payload. `Bool` is stored as one byte per
/// element (0/1), matching the wrapper-byte layout and numpy's `u1` view.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnData {
    F32(Vec<f32>),
    F64(Vec<f64>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    Bool(Vec<u8>),
}

impl ColumnData {
    /// Decode an owned contiguous column in the shared little-endian format.
    pub fn from_le_bytes(dtype: ColumnDType, bytes: &[u8]) -> Option<Self> {
        let width = match dtype {
            ColumnDType::F32 | ColumnDType::I32 | ColumnDType::U32 => 4,
            ColumnDType::F64 | ColumnDType::I64 | ColumnDType::U64 => 8,
            ColumnDType::Bool => 1,
        };
        if !bytes.len().is_multiple_of(width) {
            return None;
        }

        Some(match dtype {
            ColumnDType::F32 => ColumnData::F32(
                bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect(),
            ),
            ColumnDType::F64 => ColumnData::F64(
                bytes
                    .chunks_exact(8)
                    .map(|chunk| f64::from_le_bytes(chunk.try_into().unwrap()))
                    .collect(),
            ),
            ColumnDType::I32 => ColumnData::I32(
                bytes
                    .chunks_exact(4)
                    .map(|chunk| i32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect(),
            ),
            ColumnDType::I64 => ColumnData::I64(
                bytes
                    .chunks_exact(8)
                    .map(|chunk| i64::from_le_bytes(chunk.try_into().unwrap()))
                    .collect(),
            ),
            ColumnDType::U32 => ColumnData::U32(
                bytes
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect(),
            ),
            ColumnDType::U64 => ColumnData::U64(
                bytes
                    .chunks_exact(8)
                    .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
                    .collect(),
            ),
            ColumnDType::Bool => ColumnData::Bool(bytes.to_vec()),
        })
    }

    /// Element count (flat, before dividing by `cols`).
    pub fn len(&self) -> usize {
        match self {
            ColumnData::F32(v) => v.len(),
            ColumnData::F64(v) => v.len(),
            ColumnData::I32(v) => v.len(),
            ColumnData::I64(v) => v.len(),
            ColumnData::U32(v) => v.len(),
            ColumnData::U64(v) => v.len(),
            ColumnData::Bool(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dtype(&self) -> ColumnDType {
        match self {
            ColumnData::F32(_) => ColumnDType::F32,
            ColumnData::F64(_) => ColumnDType::F64,
            ColumnData::I32(_) => ColumnDType::I32,
            ColumnData::I64(_) => ColumnDType::I64,
            ColumnData::U32(_) => ColumnDType::U32,
            ColumnData::U64(_) => ColumnDType::U64,
            ColumnData::Bool(_) => ColumnDType::Bool,
        }
    }
}

/// One normalized `batch` column: contiguous row-major data with
/// `rows = data.len() / cols`.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchColumn {
    pub cols: usize,
    pub data: ColumnData,
}

impl BatchColumn {
    /// Entity/row count carried by this column.
    pub fn rows(&self) -> usize {
        self.data.len().checked_div(self.cols).unwrap_or(0)
    }

    /// The payload as an `f32` slice, `Some` only for `ColumnData::F32`. Native
    /// macro `batch` columns are always f32; this is their fast accessor.
    pub fn as_f32(&self) -> Option<&[f32]> {
        match &self.data {
            ColumnData::F32(v) => Some(v),
            _ => None,
        }
    }
}

/// Native macro `batch` column validation errors. `Display` reproduces the
/// pyo3 macro's strings verbatim so no observable CPython message changes.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchColumnError {
    UnknownField {
        field: String,
        valid: String,
    },
    ExpectsScalar1D {
        field: String,
        ndim: usize,
    },
    NotDivisible {
        field: String,
        cols: usize,
        len: usize,
    },
    WrongColumns {
        field: String,
        cols: usize,
        got: usize,
    },
    Not1DOr2D {
        field: String,
        ndim: usize,
    },
    LengthMismatch {
        first_field: String,
        first_rows: usize,
        field: String,
        rows: usize,
    },
    NoFields,
}

impl fmt::Display for BatchColumnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BatchColumnError::UnknownField { field, valid } => {
                write!(f, "Unknown field '{field}'. Valid fields: {valid}")
            }
            BatchColumnError::ExpectsScalar1D { field, ndim } => {
                write!(f, "Field '{field}' expects a 1D array, got {ndim}D")
            }
            BatchColumnError::NotDivisible { field, cols, len } => write!(
                f,
                "Field '{field}' requires {cols} columns, but 1D array length {len} is not divisible"
            ),
            BatchColumnError::WrongColumns { field, cols, got } => {
                write!(f, "Field '{field}' expects {cols} columns, got {got}")
            }
            BatchColumnError::Not1DOr2D { field, ndim } => {
                write!(f, "Field '{field}' must be a 1D or 2D array, got {ndim}D")
            }
            BatchColumnError::LengthMismatch {
                first_field,
                first_rows,
                field,
                rows,
            } => write!(
                f,
                "Array length mismatch: '{first_field}' has {first_rows} rows but '{field}' has {rows}"
            ),
            BatchColumnError::NoFields => {
                write!(f, "batch() requires at least one field array")
            }
        }
    }
}

impl std::error::Error for BatchColumnError {}

/// Custom `@component` `batch` validation errors. `Display` reproduces the
/// pyo3 `src/ecs/custom_batch.rs` strings verbatim. Kept SEPARATE from
/// `BatchColumnError` (note "elements" vs "rows" in the mismatch, and the
/// composite messages) so neither backend can drift.
#[derive(Debug, Clone, PartialEq)]
pub enum CustomColumnError {
    NotDecorated {
        class_name: String,
    },
    PyObjectStorage {
        class_name: String,
    },
    NoKwargs,
    UnknownField {
        field: String,
        component: String,
        valid: String,
    },
    CompositeNot2D {
        field: String,
        type_name: String,
        cols: usize,
        ndim: usize,
    },
    CompositeWrongCols {
        field: String,
        type_name: String,
        cols: usize,
        got: usize,
    },
    ScalarNot1D {
        field: String,
        ndim: usize,
    },
    LengthMismatch {
        first_field: String,
        first_len: usize,
        field: String,
        len: usize,
    },
}

impl CustomColumnError {
    /// pyo3 raises `TypeError` for the not-decorated and PyObject-storage
    /// variants, `ValueError` for the rest.
    pub fn is_type_error(&self) -> bool {
        matches!(
            self,
            CustomColumnError::NotDecorated { .. } | CustomColumnError::PyObjectStorage { .. }
        )
    }
}

impl fmt::Display for CustomColumnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CustomColumnError::NotDecorated { class_name } => {
                write!(f, "Class '{class_name}' must be decorated with @component")
            }
            // The `\` continuation in the pyo3 source renders as a single space.
            CustomColumnError::PyObjectStorage { class_name } => write!(
                f,
                "batch() is not supported for components with storage=\"python\". \
                 '{class_name}' uses PyObject storage which cannot be batch-spawned from numpy arrays."
            ),
            CustomColumnError::NoKwargs => {
                write!(f, "batch() requires at least one keyword argument")
            }
            CustomColumnError::UnknownField {
                field,
                component,
                valid,
            } => write!(
                f,
                "Unknown field '{field}' for component '{component}'. Valid fields: {valid}"
            ),
            CustomColumnError::CompositeNot2D {
                field,
                type_name,
                cols,
                ndim,
            } => write!(
                f,
                "Field '{field}' ({type_name}): expected 2D array with shape (N, {cols}), got {ndim}D array"
            ),
            CustomColumnError::CompositeWrongCols {
                field,
                type_name,
                cols,
                got,
            } => write!(
                f,
                "Field '{field}' ({type_name}): expected shape (N, {cols}), got (N, {got})"
            ),
            CustomColumnError::ScalarNot1D { field, ndim } => {
                write!(f, "Field '{field}' must be a 1D array, got {ndim}D")
            }
            CustomColumnError::LengthMismatch {
                first_field,
                first_len,
                field,
                len,
            } => write!(
                f,
                "Array length mismatch: '{first_field}' has {first_len} elements but '{field}' has {len}"
            ),
        }
    }
}

impl std::error::Error for CustomColumnError {}

/// Reject a kwarg that is not one of `valid`. The `{valid}` rendering matches
/// the pyo3 macro's `{:?}` on the field-name list.
pub fn check_known_field(field: &str, valid: &[&str]) -> Result<(), BatchColumnError> {
    if valid.contains(&field) {
        Ok(())
    } else {
        Err(BatchColumnError::UnknownField {
            field: field.to_string(),
            valid: format!("{valid:?}"),
        })
    }
}

/// Integer type of a `batch` column whose inputs must be checked before casting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerColumn {
    I32,
    I64,
    U32,
    U64,
}

impl IntegerColumn {
    pub fn from_dtype(dtype: ColumnDType) -> Option<Self> {
        match dtype {
            ColumnDType::I32 => Some(Self::I32),
            ColumnDType::I64 => Some(Self::I64),
            ColumnDType::U32 => Some(Self::U32),
            ColumnDType::U64 => Some(Self::U64),
            ColumnDType::F32 | ColumnDType::F64 | ColumnDType::Bool => None,
        }
    }

    /// Inclusive bounds of the column type.
    pub const fn bounds(self) -> (i128, i128) {
        match self {
            Self::I32 => (i32::MIN as i128, i32::MAX as i128),
            Self::I64 => (i64::MIN as i128, i64::MAX as i128),
            Self::U32 => (0, u32::MAX as i128),
            Self::U64 => (0, u64::MAX as i128),
        }
    }

    /// Human name of the type's range, as messages spell it.
    pub fn range_name(self) -> &'static str {
        match self {
            Self::I32 => "32-bit signed integer",
            Self::I64 => "64-bit signed integer",
            Self::U32 => "32-bit unsigned integer",
            Self::U64 => "64-bit unsigned integer",
        }
    }
}

/// One input element of an integer `batch` column, as the caller supplied it.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchScalar {
    Int(i128),
    /// An integer beyond `i128`, shown by its decimal text.
    BigInt(String),
    Float(f64),
    /// An element with no numeric reading, shown by its repr.
    Other(String),
}

impl fmt::Display for BatchScalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(value) => write!(f, "{value}"),
            Self::Float(value) => f.write_str(&float_repr(*value)),
            Self::BigInt(text) | Self::Other(text) => f.write_str(text),
        }
    }
}

/// Why an integer `batch` element cannot be stored exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchIntegerErrorKind {
    /// Adapters raise `ValueError`.
    OutOfRange(IntegerColumn),
    /// Fractional, NaN, infinite or non-numeric; adapters raise `TypeError`.
    NotInteger,
    /// Changed by the `f32` column a batch carries; adapters raise `ValueError`.
    InexactFloat32,
}

/// An integer `batch` element the target column cannot hold exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchIntegerError {
    pub name: String,
    pub row: Option<usize>,
    pub value: BatchScalar,
    pub kind: BatchIntegerErrorKind,
}

impl BatchIntegerError {
    pub fn is_out_of_range(&self) -> bool {
        matches!(self.kind, BatchIntegerErrorKind::OutOfRange(_))
    }

    /// Whether adapters raise `ValueError` rather than `TypeError`.
    pub fn is_value_error(&self) -> bool {
        !matches!(self.kind, BatchIntegerErrorKind::NotInteger)
    }

    /// The same error reported at `row` of the column.
    pub fn at_row(mut self, row: usize) -> Self {
        self.row = Some(row);
        self
    }
}

impl fmt::Display for BatchIntegerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            name, row, value, ..
        } = self;
        match self.kind {
            BatchIntegerErrorKind::OutOfRange(column) => {
                write!(f, "{name} must be within the {} range", column.range_name())?
            }
            BatchIntegerErrorKind::NotInteger => write!(f, "{name} must be an integer")?,
            BatchIntegerErrorKind::InexactFloat32 => write!(
                f,
                "{name} must be exactly representable as a 32-bit float in a batch column"
            )?,
        }
        match row {
            Some(row) => write!(f, " (got {value} at row {row})"),
            None => write!(f, " (got {value})"),
        }
    }
}

impl std::error::Error for BatchIntegerError {}

/// Check that `value` converts to `column` without truncation, wrapping, or saturation.
///
/// `name` is the qualified field name used in the message, such as
/// `Bloom.max_mip_dimension`.
pub fn check_batch_integer(
    name: &str,
    column: IntegerColumn,
    value: BatchScalar,
) -> Result<(), BatchIntegerError> {
    let (low, high) = column.bounds();
    let error = |value, kind| BatchIntegerError {
        name: name.to_string(),
        row: None,
        value,
        kind,
    };
    let integer = match &value {
        BatchScalar::Int(integer) => Some(*integer),
        BatchScalar::Float(float) => {
            if !float.is_finite() || float.fract() != 0.0 {
                return Err(error(value, BatchIntegerErrorKind::NotInteger));
            }
            // Integral floats beyond i128 are out of range for every column.
            (float.abs() < 1.0e38).then_some(*float as i128)
        }
        BatchScalar::BigInt(_) => None,
        BatchScalar::Other(_) => return Err(error(value, BatchIntegerErrorKind::NotInteger)),
    };
    match integer {
        Some(integer) if (low..=high).contains(&integer) => Ok(()),
        _ => Err(error(value, BatchIntegerErrorKind::OutOfRange(column))),
    }
}

/// [`check_batch_integer`] for a column carried as `f32` and cast into the
/// field, which must store the value exactly.
pub fn check_batch_integer_f32(
    name: &str,
    column: IntegerColumn,
    value: BatchScalar,
) -> Result<(), BatchIntegerError> {
    check_batch_integer(name, column, value.clone())?;
    let integer = match value {
        BatchScalar::Int(integer) => integer,
        BatchScalar::Float(float) => float as i128,
        BatchScalar::BigInt(_) | BatchScalar::Other(_) => return Ok(()),
    };
    let carried = integer as f32;
    let stored = match column {
        IntegerColumn::I32 => carried as i32 as i128,
        IntegerColumn::I64 => carried as i64 as i128,
        IntegerColumn::U32 => carried as u32 as i128,
        IntegerColumn::U64 => carried as u64 as i128,
    };
    if stored == integer {
        return Ok(());
    }
    Err(BatchIntegerError {
        name: name.to_string(),
        row: None,
        value,
        kind: BatchIntegerErrorKind::InexactFloat32,
    })
}

/// Check values bound for a View field before any selected row is written.
///
/// Each value is checked as the field stores it after the write's cast, so an
/// integer field sees the truncated value and an `f32` field the rounded one.
pub fn check_view_assignment(
    owner: &'static str,
    field: &'static str,
    values: &[f64],
    field_type: crate::FieldType,
    domains: &'static [FieldDomain],
) -> Result<(), DomainViolation> {
    for (row, &value) in values.iter().enumerate() {
        check(owner, field, stored_value(value, field_type), domains)
            .map_err(|violation| violation.at_row(row))?;
    }
    Ok(())
}

/// The value a write of `value` leaves in a field of `field_type`.
fn stored_value(value: f64, field_type: crate::FieldType) -> DomainValue {
    use crate::FieldType;

    match field_type {
        FieldType::F32 => DomainValue::F32(value as f32),
        FieldType::I32 => DomainValue::Int(i128::from(value as i32)),
        FieldType::U8 => DomainValue::Int(i128::from(value as u8)),
        FieldType::U32 => DomainValue::Int(i128::from(value as u32)),
        _ => DomainValue::F64(value),
    }
}

/// Validate a column's shape against `cols` and return its row (entity) count.
/// Scalar fields (`cols == 1`) accept 1-D only; vector fields accept 1-D with a
/// length divisible by `cols`, or 2-D with exactly `cols` columns.
pub fn plan_column(
    field: &str,
    cols: usize,
    ndim: usize,
    shape: &[usize],
) -> Result<usize, BatchColumnError> {
    if cols == 1 {
        if ndim != 1 {
            return Err(BatchColumnError::ExpectsScalar1D {
                field: field.to_string(),
                ndim,
            });
        }
        Ok(shape.first().copied().unwrap_or(0))
    } else if ndim == 1 {
        let len = shape.first().copied().unwrap_or(0);
        if len % cols != 0 {
            return Err(BatchColumnError::NotDivisible {
                field: field.to_string(),
                cols,
                len,
            });
        }
        Ok(len / cols)
    } else if ndim == 2 {
        let rows = shape.first().copied().unwrap_or(0);
        let got = shape.get(1).copied().unwrap_or(0);
        if got != cols {
            return Err(BatchColumnError::WrongColumns {
                field: field.to_string(),
                cols,
                got,
            });
        }
        Ok(rows)
    } else {
        Err(BatchColumnError::Not1DOr2D {
            field: field.to_string(),
            ndim,
        })
    }
}

/// Which validation rule set (and error strings) a column obeys. The native
/// macro `batch` and the custom `@component` `batch` have different
/// shape rules and messages, so the shared planner is an enum over both.
pub enum ColumnShape<'a> {
    /// Native macro `batch` rules (`plan_column` + `BatchColumnError`).
    Native { field: &'a str, cols: usize },
    /// Custom `@component` scalar field: 1-D only, custom-batch message.
    CustomScalar { field: &'a str },
    /// Custom `@component` Vec3/Vec2 field: strict 2-D `(N, cols)`. `type_name`
    /// is the `{:?}` rendering of the `PrimitiveType` ("Vec3"/"Vec2").
    CustomComposite {
        field: &'a str,
        type_name: &'a str,
        cols: usize,
    },
}

impl ColumnShape<'_> {
    pub fn field(&self) -> &str {
        match self {
            ColumnShape::Native { field, .. }
            | ColumnShape::CustomScalar { field }
            | ColumnShape::CustomComposite { field, .. } => field,
        }
    }

    /// Columns per entity: `cols` for native/composite, 1 for a custom scalar.
    pub fn cols(&self) -> usize {
        match self {
            ColumnShape::Native { cols, .. } | ColumnShape::CustomComposite { cols, .. } => *cols,
            ColumnShape::CustomScalar { .. } => 1,
        }
    }

    /// Row count for a `(ndim, shape)` probe, or the rule set's exact error.
    pub fn plan(&self, ndim: usize, shape: &[usize]) -> Result<usize, ColumnPlanError> {
        match self {
            ColumnShape::Native { field, cols } => {
                plan_column(field, *cols, ndim, shape).map_err(ColumnPlanError::Native)
            }
            ColumnShape::CustomScalar { field } => {
                if ndim != 1 {
                    return Err(ColumnPlanError::Custom(CustomColumnError::ScalarNot1D {
                        field: field.to_string(),
                        ndim,
                    }));
                }
                Ok(shape.first().copied().unwrap_or(0))
            }
            ColumnShape::CustomComposite {
                field,
                type_name,
                cols,
            } => {
                if ndim != 2 {
                    return Err(ColumnPlanError::Custom(CustomColumnError::CompositeNot2D {
                        field: field.to_string(),
                        type_name: type_name.to_string(),
                        cols: *cols,
                        ndim,
                    }));
                }
                let got = shape.get(1).copied().unwrap_or(0);
                if got != *cols {
                    return Err(ColumnPlanError::Custom(
                        CustomColumnError::CompositeWrongCols {
                            field: field.to_string(),
                            type_name: type_name.to_string(),
                            cols: *cols,
                            got,
                        },
                    ));
                }
                Ok(shape.first().copied().unwrap_or(0))
            }
        }
    }
}

/// A planning error from either rule set. `Display` delegates to the wrapped
/// enum; adapters map both to `ValueError`.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnPlanError {
    Native(BatchColumnError),
    Custom(CustomColumnError),
}

impl fmt::Display for ColumnPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnPlanError::Native(e) => e.fmt(f),
            ColumnPlanError::Custom(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ColumnPlanError {}

/// Tracks cross-column row-count agreement: the first observed column fixes the
/// entity count, and every later column must match it.
#[derive(Debug, Default)]
pub struct CountAgreement {
    first: Option<(usize, String)>,
}

impl CountAgreement {
    pub fn observe(&mut self, field: &str, rows: usize) -> Result<(), BatchColumnError> {
        match &self.first {
            Some((first_rows, first_field)) => {
                if rows != *first_rows {
                    return Err(BatchColumnError::LengthMismatch {
                        first_field: first_field.clone(),
                        first_rows: *first_rows,
                        field: field.to_string(),
                        rows,
                    });
                }
                Ok(())
            }
            None => {
                self.first = Some((rows, field.to_string()));
                Ok(())
            }
        }
    }

    /// The agreed entity count, or `None` if no column was observed.
    pub fn count(&self) -> Option<usize> {
        self.first.as_ref().map(|(rows, _)| *rows)
    }
}

/// Cross-column row-count agreement for custom `@component` batches. Mirrors
/// `CountAgreement` but emits `CustomColumnError::LengthMismatch` (the pyo3
/// custom-batch "elements" wording).
#[derive(Debug, Default)]
pub struct CustomCountAgreement {
    first: Option<(usize, String)>,
}

impl CustomCountAgreement {
    pub fn observe(&mut self, field: &str, len: usize) -> Result<(), CustomColumnError> {
        match &self.first {
            Some((first_len, first_field)) => {
                if len != *first_len {
                    return Err(CustomColumnError::LengthMismatch {
                        first_field: first_field.clone(),
                        first_len: *first_len,
                        field: field.to_string(),
                        len,
                    });
                }
                Ok(())
            }
            None => {
                self.first = Some((len, field.to_string()));
                Ok(())
            }
        }
    }

    /// The agreed entity count, or `None` if no column was observed.
    pub fn count(&self) -> Option<usize> {
        self.first.as_ref().map(|(len, _)| *len)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::FieldType;

    #[test]
    fn owned_column_decodes_little_endian_values() {
        assert_eq!(
            ColumnData::from_le_bytes(ColumnDType::F32, &1.5f32.to_le_bytes()),
            Some(ColumnData::F32(vec![1.5]))
        );
        assert_eq!(
            ColumnData::from_le_bytes(ColumnDType::I64, &(-7i64).to_le_bytes()),
            Some(ColumnData::I64(vec![-7]))
        );
        assert_eq!(
            ColumnData::from_le_bytes(ColumnDType::Bool, &[0, 1, 255]),
            Some(ColumnData::Bool(vec![0, 1, 255]))
        );
        assert_eq!(ColumnData::from_le_bytes(ColumnDType::F32, &[0, 1]), None);
    }

    #[test]
    fn scalar_field_accepts_1d_only() {
        assert_eq!(plan_column("intensity", 1, 1, &[3]).unwrap(), 3);
        assert_eq!(
            plan_column("intensity", 1, 2, &[3, 1])
                .unwrap_err()
                .to_string(),
            "Field 'intensity' expects a 1D array, got 2D"
        );
    }

    #[test]
    fn vector_field_1d_divisibility_and_2d_columns() {
        assert_eq!(plan_column("translation", 3, 1, &[6]).unwrap(), 2);
        assert_eq!(
            plan_column("translation", 3, 1, &[7])
                .unwrap_err()
                .to_string(),
            "Field 'translation' requires 3 columns, but 1D array length 7 is not divisible"
        );
        assert_eq!(plan_column("translation", 3, 2, &[2, 3]).unwrap(), 2);
        assert_eq!(
            plan_column("translation", 3, 2, &[2, 4])
                .unwrap_err()
                .to_string(),
            "Field 'translation' expects 3 columns, got 4"
        );
        assert_eq!(
            plan_column("translation", 3, 3, &[2, 3, 1])
                .unwrap_err()
                .to_string(),
            "Field 'translation' must be a 1D or 2D array, got 3D"
        );
    }

    #[test]
    fn unknown_field_message() {
        assert_eq!(
            check_known_field("bogus", &["intensity", "range"])
                .unwrap_err()
                .to_string(),
            "Unknown field 'bogus'. Valid fields: [\"intensity\", \"range\"]"
        );
        assert!(check_known_field("range", &["intensity", "range"]).is_ok());
    }

    const UNIT_RANGE: &[FieldDomain] = &[FieldDomain::Range { lo: 0.0, hi: 1.0 }];
    const POSITIVE: &[FieldDomain] = &[FieldDomain::Finite, FieldDomain::Positive];
    const SYMMETRIC: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: -1.0, hi: 1.0 }];

    #[test]
    fn view_assignments_check_the_value_the_field_stores() {
        let f64_field =
            check_view_assignment("T", "f", &[0.999_999_999_9], FieldType::F64, UNIT_RANGE);
        assert!(f64_field.is_ok());
        let f32_field =
            check_view_assignment("T", "f", &[0.999_999_999_9], FieldType::F32, UNIT_RANGE);
        assert_eq!(
            f32_field.map_err(|violation| violation.to_string()),
            Err("T.f must be in [0.0, 1.0) (got 1.0 at row 0)".into())
        );
    }

    #[test]
    fn integer_view_fields_check_the_truncated_value() {
        assert!(check_view_assignment("Bloom", "n", &[1.0, 7.9], FieldType::U32, POSITIVE).is_ok());
        for value in [0.5, -3.0, f64::NAN] {
            assert_eq!(
                check_view_assignment("Bloom", "n", &[value], FieldType::U32, POSITIVE)
                    .map_err(|violation| violation.to_string()),
                Err("Bloom.n must be greater than zero (got 0 at row 0)".into())
            );
        }
    }

    #[test]
    fn view_assignment_reports_the_first_failing_row() {
        assert_eq!(
            check_view_assignment(
                "FogVolume",
                "a",
                &[0.5, 2.0, 3.0],
                FieldType::F32,
                SYMMETRIC
            )
            .map_err(|violation| violation.to_string()),
            Err("FogVolume.a must be in [-1.0, 1.0] (got 2.0 at row 1)".into())
        );
        assert!(check_view_assignment("T", "f", &[], FieldType::F32, SYMMETRIC).is_ok());
    }

    #[test]
    fn count_agreement_and_mismatch() {
        let mut agree = CountAgreement::default();
        agree.observe("intensity", 3).unwrap();
        assert_eq!(agree.count(), Some(3));
        agree.observe("range", 3).unwrap();
        assert_eq!(
            agree.observe("radius", 5).unwrap_err().to_string(),
            "Array length mismatch: 'intensity' has 3 rows but 'radius' has 5"
        );
    }

    #[test]
    fn no_fields_message() {
        assert_eq!(
            BatchColumnError::NoFields.to_string(),
            "batch() requires at least one field array"
        );
    }

    #[test]
    fn batch_column_rows() {
        assert_eq!(
            BatchColumn {
                cols: 3,
                data: ColumnData::F32(vec![0.0; 6])
            }
            .rows(),
            2
        );
        assert_eq!(
            BatchColumn {
                cols: 1,
                data: ColumnData::F32(vec![0.0; 4])
            }
            .rows(),
            4
        );
        assert_eq!(
            BatchColumn {
                cols: 1,
                data: ColumnData::I64(vec![0; 3])
            }
            .rows(),
            3
        );
    }

    #[test]
    fn column_data_dtype_and_len() {
        assert_eq!(ColumnData::I64(vec![1, 2, 3]).dtype(), ColumnDType::I64);
        assert_eq!(ColumnData::Bool(vec![1, 0]).len(), 2);
        assert!(ColumnData::F64(vec![]).is_empty());
    }

    #[test]
    fn batch_column_as_f32() {
        assert_eq!(
            BatchColumn {
                cols: 1,
                data: ColumnData::F32(vec![1.0, 2.0])
            }
            .as_f32(),
            Some([1.0f32, 2.0].as_slice())
        );
        assert_eq!(
            BatchColumn {
                cols: 1,
                data: ColumnData::I32(vec![1])
            }
            .as_f32(),
            None
        );
    }

    #[test]
    fn custom_scalar_shape_rule() {
        let rule = ColumnShape::CustomScalar { field: "hits" };
        assert_eq!(rule.cols(), 1);
        assert_eq!(rule.plan(1, &[4]).unwrap(), 4);
        assert_eq!(
            rule.plan(2, &[4, 1]).unwrap_err().to_string(),
            "Field 'hits' must be a 1D array, got 2D"
        );
    }

    #[test]
    fn custom_composite_shape_rule() {
        let rule = ColumnShape::CustomComposite {
            field: "pos",
            type_name: "Vec3",
            cols: 3,
        };
        assert_eq!(rule.cols(), 3);
        assert_eq!(rule.plan(2, &[2, 3]).unwrap(), 2);
        assert_eq!(
            rule.plan(1, &[6]).unwrap_err().to_string(),
            "Field 'pos' (Vec3): expected 2D array with shape (N, 3), got 1D array"
        );
        assert_eq!(
            rule.plan(2, &[2, 2]).unwrap_err().to_string(),
            "Field 'pos' (Vec3): expected shape (N, 3), got (N, 2)"
        );
    }

    #[test]
    fn native_shape_rule_delegates() {
        let rule = ColumnShape::Native {
            field: "translation",
            cols: 3,
        };
        assert_eq!(rule.plan(2, &[2, 3]).unwrap(), 2);
        assert_eq!(
            rule.plan(3, &[2, 3, 1]).unwrap_err().to_string(),
            "Field 'translation' must be a 1D or 2D array, got 3D"
        );
    }

    #[test]
    fn custom_errors_verbatim_and_exception_kind() {
        assert_eq!(
            CustomColumnError::NotDecorated {
                class_name: "Foo".into()
            }
            .to_string(),
            "Class 'Foo' must be decorated with @component"
        );
        assert!(
            CustomColumnError::NotDecorated {
                class_name: "Foo".into()
            }
            .is_type_error()
        );
        assert_eq!(
            CustomColumnError::PyObjectStorage {
                class_name: "Foo".into()
            }
            .to_string(),
            "batch() is not supported for components with storage=\"python\". \
             'Foo' uses PyObject storage which cannot be batch-spawned from numpy arrays."
        );
        assert!(
            CustomColumnError::PyObjectStorage {
                class_name: "Foo".into()
            }
            .is_type_error()
        );
        assert_eq!(
            CustomColumnError::NoKwargs.to_string(),
            "batch() requires at least one keyword argument"
        );
        assert!(!CustomColumnError::NoKwargs.is_type_error());
        assert_eq!(
            CustomColumnError::UnknownField {
                field: "bogus".into(),
                component: "Particle".into(),
                valid: "[\"x\", \"y\"]".into(),
            }
            .to_string(),
            "Unknown field 'bogus' for component 'Particle'. Valid fields: [\"x\", \"y\"]"
        );
    }

    #[test]
    fn custom_count_agreement_uses_elements_wording() {
        let mut agree = CustomCountAgreement::default();
        agree.observe("x", 3).unwrap();
        assert_eq!(agree.count(), Some(3));
        assert_eq!(
            agree.observe("y", 5).unwrap_err().to_string(),
            "Array length mismatch: 'x' has 3 elements but 'y' has 5"
        );
    }

    #[test]
    fn native_error_display_matches_macro_wording() {
        assert_eq!(
            BatchColumnError::ExpectsScalar1D {
                field: "intensity".into(),
                ndim: 2
            }
            .to_string(),
            "Field 'intensity' expects a 1D array, got 2D"
        );
        assert_eq!(
            BatchColumnError::Not1DOr2D {
                field: "translation".into(),
                ndim: 3
            }
            .to_string(),
            "Field 'translation' must be a 1D or 2D array, got 3D"
        );
        assert_eq!(
            BatchColumnError::WrongColumns {
                field: "translation".into(),
                cols: 3,
                got: 4
            }
            .to_string(),
            "Field 'translation' expects 3 columns, got 4"
        );
        assert_eq!(
            BatchColumnError::NotDivisible {
                field: "translation".into(),
                cols: 3,
                len: 4
            }
            .to_string(),
            "Field 'translation' requires 3 columns, but 1D array length 4 is not divisible"
        );
        assert_eq!(
            BatchColumnError::NoFields.to_string(),
            "batch() requires at least one field array"
        );
    }

    #[test]
    fn column_shape_field_accessor_covers_every_variant() {
        assert_eq!(
            ColumnShape::Native {
                field: "translation",
                cols: 3
            }
            .field(),
            "translation"
        );
        assert_eq!(ColumnShape::CustomScalar { field: "hits" }.field(), "hits");
        assert_eq!(
            ColumnShape::CustomComposite {
                field: "pos",
                type_name: "Vec3",
                cols: 3
            }
            .field(),
            "pos"
        );
    }

    #[test]
    fn column_data_dtype_maps_every_variant() {
        assert_eq!(ColumnData::F32(vec![0.0]).dtype(), ColumnDType::F32);
        assert_eq!(ColumnData::F64(vec![0.0]).dtype(), ColumnDType::F64);
        assert_eq!(ColumnData::I32(vec![0]).dtype(), ColumnDType::I32);
        assert_eq!(ColumnData::I64(vec![0]).dtype(), ColumnDType::I64);
        assert_eq!(ColumnData::U32(vec![0]).dtype(), ColumnDType::U32);
        assert_eq!(ColumnData::U64(vec![0]).dtype(), ColumnDType::U64);
        assert_eq!(ColumnData::Bool(vec![1]).dtype(), ColumnDType::Bool);
    }

    #[test]
    fn empty_shapes_plan_zero_rows_and_non_divisible_columns_plan_zero() {
        assert_eq!(plan_column("intensity", 1, 1, &[]).unwrap(), 0);
        assert_eq!(plan_column("translation", 3, 1, &[]).unwrap(), 0);
        // An empty 2-D shape reports its zero column count, like any
        // non-matching 2-D shape.
        assert_eq!(
            plan_column("translation", 3, 2, &[])
                .unwrap_err()
                .to_string(),
            "Field 'translation' expects 3 columns, got 0"
        );

        let column = BatchColumn {
            cols: 3,
            data: ColumnData::F32(vec![0.0, 0.0]),
        };
        assert_eq!(column.rows(), 0);
        assert_eq!(
            BatchColumn {
                cols: 2,
                data: ColumnData::F32(vec![1.0, 2.0, 3.0, 4.0])
            }
            .rows(),
            2
        );
        assert!(
            BatchColumn {
                cols: 1,
                data: ColumnData::I32(vec![7])
            }
            .as_f32()
            .is_none()
        );
    }

    #[test]
    fn f32_integer_check_rejects_only_rounded_values() {
        let check = |value| check_batch_integer_f32("C.f", IntegerColumn::U32, value);
        for exact in [16_777_216, 33_554_432, i128::from(u32::MAX)] {
            assert!(check(BatchScalar::Int(exact)).is_ok(), "{exact}");
        }
        assert!(check(BatchScalar::Float(16_777_216.0)).is_ok());

        let rounded = check(BatchScalar::Int(16_777_217)).unwrap_err();
        assert!(rounded.is_value_error());
        assert!(!rounded.is_out_of_range());
        assert_eq!(
            rounded.at_row(1).to_string(),
            "C.f must be exactly representable as a 32-bit float in a batch column \
             (got 16777217 at row 1)"
        );

        let out_of_range = check(BatchScalar::Int(-1)).unwrap_err();
        assert!(out_of_range.is_out_of_range());
        let fraction = check(BatchScalar::Float(0.5)).unwrap_err();
        assert!(!fraction.is_value_error());

        let signed =
            |value| check_batch_integer_f32("C.f", IntegerColumn::I32, BatchScalar::Int(value));
        assert!(signed(i32::MAX.into()).is_ok());
        assert!(signed(i32::MIN.into()).is_ok());
        assert!(signed(-16_777_217).is_err());
    }

    #[test]
    fn integer_check_rejects_values_the_column_cannot_hold() {
        let check = |column, value| check_batch_integer("C.f", column, value);
        assert!(check(IntegerColumn::U32, BatchScalar::Int(0)).is_ok());
        assert!(check(IntegerColumn::U32, BatchScalar::Int(u32::MAX.into())).is_ok());
        assert!(check(IntegerColumn::I32, BatchScalar::Float(-3.0)).is_ok());

        let negative = check(IntegerColumn::U32, BatchScalar::Int(-1)).unwrap_err();
        assert!(negative.is_out_of_range());
        assert_eq!(
            negative.clone().at_row(2).to_string(),
            "C.f must be within the 32-bit unsigned integer range (got -1 at row 2)"
        );
        assert_eq!(
            negative.to_string(),
            "C.f must be within the 32-bit unsigned integer range (got -1)"
        );
        let oversized = check(IntegerColumn::U32, BatchScalar::Int(1 << 40)).unwrap_err();
        assert!(oversized.is_out_of_range());
        assert!(
            check(IntegerColumn::U64, BatchScalar::Float(1e30))
                .unwrap_err()
                .is_out_of_range()
        );
        assert!(
            check(IntegerColumn::I64, BatchScalar::BigInt("2".repeat(50)))
                .unwrap_err()
                .is_out_of_range()
        );
        assert!(
            !check(IntegerColumn::I64, BatchScalar::Other("'a'".into()))
                .unwrap_err()
                .is_out_of_range()
        );

        let fraction = check(IntegerColumn::I64, BatchScalar::Float(7.9)).unwrap_err();
        assert!(!fraction.is_out_of_range());
        assert_eq!(fraction.to_string(), "C.f must be an integer (got 7.9)");
        assert_eq!(
            check(IntegerColumn::U64, BatchScalar::Float(1e30))
                .unwrap_err()
                .to_string(),
            "C.f must be within the 64-bit unsigned integer range (got 1e+30)"
        );
        assert!(
            !check(IntegerColumn::I64, BatchScalar::Float(f64::NAN))
                .unwrap_err()
                .is_out_of_range()
        );
    }
}
