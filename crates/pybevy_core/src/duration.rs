use std::time::Duration;

use pyo3::{exceptions::PyTypeError, prelude::*};

/// Convert a Python `timedelta`, `float` (seconds), or `int` (seconds) to a `Duration`.
pub fn duration_from_py(value: &Bound<'_, PyAny>) -> PyResult<Duration> {
    if let Ok(duration) = value.extract::<Duration>() {
        return Ok(duration);
    }

    if let Ok(seconds) = value.extract::<f64>() {
        if seconds < 0.0 {
            return Err(PyTypeError::new_err("Duration cannot be negative"));
        }
        return Ok(Duration::from_secs_f64(seconds));
    }

    if let Ok(seconds) = value.extract::<u64>() {
        return Ok(Duration::from_secs(seconds));
    }

    Err(PyTypeError::new_err(
        "Duration must be a Duration object, float (seconds), or int (seconds)",
    ))
}
