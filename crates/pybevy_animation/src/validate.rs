//! Finiteness checks for animation parameters.

use pybevy_core::public_error;
use pyo3::{exceptions::PyValueError, prelude::*};

pub fn validate_finite(value: f32, parameter: &str) -> PyResult<f32> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(PyValueError::new_err(
            public_error::animation_parameter_must_be_finite(parameter, value),
        ))
    }
}
