//! Shared `__richcmp__` result binding for the math value wrappers.

use pyo3::{prelude::*, types::PyBool};

/// Bind a comparison outcome as an owned Python `bool`.
///
/// The wrappers return `Py<PyAny>` so a type mismatch can answer
/// `NotImplemented` and let Python fall back to identity and the reflected
/// operand, which a `bool` return cannot express.
pub(crate) fn comparison_result(py: Python<'_>, result: bool) -> Py<PyAny> {
    PyBool::new(py, result).to_owned().into_any().unbind()
}
