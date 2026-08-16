use std::{path::PathBuf, time::Duration};

use pybevy_reload::{DEFAULT_IGNORE_PATTERNS, FileWatcher};
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};

/// Private PyO3 adapter for the interpreter-neutral file watcher.
///
/// The Python CLI owns this adapter on its existing watcher thread. The
/// `FileWatcher` worker itself contains no Python objects.
#[pyclass(name = "_FileWatcher")]
pub struct PyFileWatcher {
    watcher: FileWatcher,
}

#[pymethods]
impl PyFileWatcher {
    #[new]
    #[pyo3(signature = (paths, ignore_patterns = None, debounce_ms = 50))]
    fn new(
        paths: Vec<String>,
        ignore_patterns: Option<Vec<String>>,
        debounce_ms: u64,
    ) -> PyResult<Self> {
        let roots = paths.into_iter().map(PathBuf::from).collect();
        let ignore_patterns = ignore_patterns.unwrap_or_else(|| {
            DEFAULT_IGNORE_PATTERNS
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect()
        });
        let watcher =
            FileWatcher::start(roots, ignore_patterns, Duration::from_millis(debounce_ms))
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self { watcher })
    }

    #[pyo3(signature = (timeout_seconds = 0.1))]
    fn poll(&self, py: Python<'_>, timeout_seconds: f64) -> PyResult<Option<Vec<String>>> {
        if !timeout_seconds.is_finite() || !(0.0..=60.0).contains(&timeout_seconds) {
            return Err(PyValueError::new_err(
                "timeout_seconds must be finite and between 0 and 60",
            ));
        }
        let timeout = Duration::from_secs_f64(timeout_seconds);
        let batch = py
            .detach(|| self.watcher.recv_timeout(timeout))
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(batch.map(|batch| {
            batch
                .into_paths()
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect()
        }))
    }

    fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.watcher.stop())
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }
}
