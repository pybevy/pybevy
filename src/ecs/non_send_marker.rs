use pybevy_core::public_error::injected_system_parameter;
use pyo3::{exceptions::PyTypeError, prelude::*};

/// System parameter that pins its system to the main thread, as Bevy's
/// `NonSendMarker` does.
#[pyclass(name = "NonSendMarker", module = "pybevy.ecs", frozen)]
pub struct PyNonSendMarker;

#[pymethods]
impl PyNonSendMarker {
    #[new]
    fn new_injected_only() -> PyResult<Self> {
        Err(PyTypeError::new_err(injected_system_parameter(
            "NonSendMarker",
            "_main_thread: NonSendMarker",
        )))
    }

    fn __repr__(&self) -> &'static str {
        "NonSendMarker"
    }
}
