use pybevy_core::public_error::INTEGER_VECTOR_OVERFLOW;
use pyo3::{exceptions::PyOverflowError, prelude::*};

pub(crate) fn checked<T>(value: Option<T>) -> PyResult<T> {
    value.ok_or_else(|| PyOverflowError::new_err(INTEGER_VECTOR_OVERFLOW))
}
