pub(crate) use pybevy_reload::is_verbose;
use pyo3::prelude::*;

/// Get the number of Python objects currently tracked by the cyclic GC.
pub(crate) fn get_python_gc_objects() -> usize {
    Python::attach(|py| {
        if let Ok(gc) = py.import("gc")
            && let Ok(objects) = gc.call_method0("get_objects")
            && let Ok(count) = objects.len()
        {
            return count;
        }
        0
    })
}

/// Detect if Python GIL is enabled (Python 3.13+ free-threading detection)
pub(crate) fn detect_gil_status() -> bool {
    Python::attach(|py| {
        if let Ok(sys) = py.import("sys")
            && let Ok(enabled_attr) = sys.getattr("_is_gil_enabled")
            && let Ok(result) = enabled_attr.call0()
        {
            return result.extract().unwrap_or(true);
        }
        true
    })
}
