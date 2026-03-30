pub(crate) use pybevy_reload::is_verbose;
use pyo3::prelude::*;

/// Get total Python GC tracked objects (sum of gc.get_count() across all generations).
pub(crate) fn get_python_gc_objects() -> usize {
    Python::attach(|py| {
        if let Ok(gc) = py.import("gc")
            && let Ok(counts) = gc.call_method0("get_count")
            && let Ok(tuple) = counts.extract::<(usize, usize, usize)>()
        {
            return tuple.0 + tuple.1 + tuple.2;
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
