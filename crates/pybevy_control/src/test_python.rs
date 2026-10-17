use std::sync::Once;

use ::pyo3::{prelude::*, types::PyModule};

static INIT: Once = Once::new();

/// Starts the interpreter and imports the stdlib modules conversions use, so
/// parallel tests never race on a module's first import.
pub(crate) fn initialize_python() {
    INIT.call_once(|| {
        Python::initialize();
        Python::attach(|py| {
            for module in ["typing", "dataclasses", "inspect", "types"] {
                PyModule::import(py, module).expect("stdlib module imports");
            }
        });
    });
}
