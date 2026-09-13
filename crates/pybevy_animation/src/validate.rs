//! Finiteness checks for animation parameters.

use pyo3::{exceptions::PyValueError, prelude::*};

pub fn validate_finite(value: f32, parameter: &str) -> PyResult<f32> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(PyValueError::new_err(format!(
            "{parameter} must be finite (got {value})"
        )))
    }
}
