//! Neutral scalar values used to read and write elements without committing to
//! a concrete Rust primitive at the call site.

/// A dtype-agnostic element value.
///
/// Floats are carried as `f64`; every supported integer/unsigned dtype fits in
/// `i64` exactly, and booleans are distinct so bool arrays never round-trip
/// through a numeric type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scalar {
    F64(f64),
    I64(i64),
    Bool(bool),
}

impl Scalar {
    /// Interpret the value as `f64` (bool -> 0.0/1.0).
    pub fn to_f64(self) -> f64 {
        match self {
            Scalar::F64(v) => v,
            Scalar::I64(v) => v as f64,
            Scalar::Bool(v) => f64::from(v as u8),
        }
    }

    /// Truncate toward zero to `i64`, matching NumPy float->int cast semantics.
    pub fn to_i64_trunc(self) -> i64 {
        match self {
            Scalar::F64(v) => v as i64,
            Scalar::I64(v) => v,
            Scalar::Bool(v) => i64::from(v),
        }
    }

    /// Truthiness, matching NumPy's nonzero rule (NaN is truthy).
    pub fn to_bool(self) -> bool {
        match self {
            Scalar::F64(v) => v != 0.0,
            Scalar::I64(v) => v != 0,
            Scalar::Bool(v) => v,
        }
    }
}

impl From<f64> for Scalar {
    fn from(v: f64) -> Self {
        Scalar::F64(v)
    }
}

impl From<i64> for Scalar {
    fn from(v: i64) -> Self {
        Scalar::I64(v)
    }
}

impl From<bool> for Scalar {
    fn from(v: bool) -> Self {
        Scalar::Bool(v)
    }
}
