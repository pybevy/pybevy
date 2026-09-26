use pyo3::{exceptions::PyValueError, prelude::*};

pub(super) fn minimum_count(name: &str, count: u32, minimum: u32) -> PyResult<()> {
    if count < minimum {
        return Err(PyValueError::new_err(format!(
            "{name} must be at least {minimum}"
        )));
    }
    Ok(())
}
