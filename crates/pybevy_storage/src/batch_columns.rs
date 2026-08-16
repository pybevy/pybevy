//! Interpreter-neutral `from_numpy` batch-column validation. Shared by the pyo3
//! component macro and the RustPython adapter so shape rules and error strings
//! cannot drift (the "validation, IR, registries, and error definitions" the
//! Backend Unification rules keep neutral). The data work (numpy
//! `reshape`/`astype` on pyo3; `astype`/`to_scalars` on RP2) stays in the
//! adapters; this module owns only shape/count validation and the error text.

use std::fmt;

/// Target dtype of one `from_numpy` column. Covers every
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

/// One normalized `from_numpy` column: contiguous row-major data with
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
    /// macro `from_numpy` columns are always f32; this is their fast accessor.
    pub fn as_f32(&self) -> Option<&[f32]> {
        match &self.data {
            ColumnData::F32(v) => Some(v),
            _ => None,
        }
    }
}

/// Native macro `from_numpy` column validation errors. `Display` reproduces the
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
    NonFinite {
        field: String,
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
            BatchColumnError::NonFinite { field } => {
                write!(f, "Field '{field}' must contain only finite values")
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
                write!(f, "from_numpy() requires at least one field array")
            }
        }
    }
}

impl std::error::Error for BatchColumnError {}

/// Value-domain constraints attached to a native component batch field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BatchValueConstraint {
    Finite,
}

/// Custom `@component` `from_numpy` validation errors. `Display` reproduces the
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
                "from_numpy() is not supported for components with storage=\"python\". \
                 '{class_name}' uses PyObject storage which cannot be batch-spawned from numpy arrays."
            ),
            CustomColumnError::NoKwargs => {
                write!(f, "from_numpy() requires at least one keyword argument")
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

/// Apply a native batch field's declared constraints to normalized float data.
pub fn validate_f32_values(
    field: &str,
    values: &[f32],
    constraints: &[BatchValueConstraint],
) -> Result<(), BatchColumnError> {
    for constraint in constraints {
        match constraint {
            BatchValueConstraint::Finite if values.iter().any(|value| !value.is_finite()) => {
                return Err(BatchColumnError::NonFinite {
                    field: field.to_string(),
                });
            }
            BatchValueConstraint::Finite => {}
        }
    }
    Ok(())
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
/// macro `from_numpy` and the custom `@component` `from_numpy` have different
/// shape rules and messages, so the shared planner is an enum over both.
pub enum ColumnShape<'a> {
    /// Native macro `from_numpy` rules (`plan_column` + `BatchColumnError`).
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
mod tests {
    use super::*;

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

    #[test]
    fn finite_column_validation() {
        let constraints = [BatchValueConstraint::Finite];
        assert!(validate_f32_values("translation", &[0.0, -1.0, 1.0e20], &constraints).is_ok());
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                validate_f32_values("translation", &[0.0, value], &constraints)
                    .unwrap_err()
                    .to_string(),
                "Field 'translation' must contain only finite values"
            );
        }
        assert!(validate_f32_values("unconstrained", &[f32::NAN], &[]).is_ok());
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
            "from_numpy() requires at least one field array"
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
            "from_numpy() is not supported for components with storage=\"python\". \
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
            "from_numpy() requires at least one keyword argument"
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
}
