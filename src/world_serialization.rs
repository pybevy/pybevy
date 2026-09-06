//! PyO3 adapter for extracting and serializing a live Bevy World.

use std::ffi::CString;

use pybevy_world_serialization::{
    dynamic_world::PyDynamicWorld,
    live_world::{LiveWorldSerializationError, extract_live_world, serialize_dynamic_world},
};
use pyo3::{
    exceptions::{PyRuntimeError, PyUserWarning},
    prelude::*,
    types::{IntoPyDict, PyModule},
};

use crate::ecs::world::PyWorld;

fn live_world_error(error: LiveWorldSerializationError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

#[pyfunction]
fn _dynamic_world_from_world(py: Python<'_>, world: &PyWorld) -> PyResult<Py<PyDynamicWorld>> {
    let extraction = extract_live_world(world.world_ref()?).map_err(live_world_error)?;
    if !extraction.skipped_custom_types.is_empty() {
        let message = extraction
            .skipped_custom_types
            .warning_message()
            .replace('\0', "\\0");
        let message = CString::new(message).expect("NUL bytes were escaped");
        PyErr::warn(py, &py.get_type::<PyUserWarning>(), &message, 2)?;
    }
    if !extraction.skipped_reflected_types.is_empty() {
        let message = extraction
            .skipped_reflected_types
            .warning_message()
            .replace('\0', "\\0");
        let message = CString::new(message).expect("NUL bytes were escaped");
        PyErr::warn(py, &py.get_type::<PyUserWarning>(), &message, 2)?;
    }
    Py::new(py, PyDynamicWorld::from_owned(extraction.dynamic_world))
}

#[pyfunction]
fn _dynamic_world_serialize(dynamic_world: &PyDynamicWorld, world: &PyWorld) -> PyResult<String> {
    let dynamic_world = dynamic_world.as_ref()?;
    serialize_dynamic_world(&dynamic_world, world.world_ref()?).map_err(live_world_error)
}

pub(crate) fn add_api(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let module = parent
        .getattr("world_serialization")?
        .cast_into::<PyModule>()?;
    module.add_function(wrap_pyfunction!(_dynamic_world_from_world, &module)?)?;
    module.add_function(wrap_pyfunction!(_dynamic_world_serialize, &module)?)?;

    let dynamic_world = module.getattr("DynamicWorld")?;
    let from_world = module.getattr("_dynamic_world_from_world")?;
    let serialize = module.getattr("_dynamic_world_serialize")?;
    py.run(
        c"DynamicWorld.from_world = staticmethod(_from_world)\nDynamicWorld.serialize = lambda self, world: _serialize(self, world)",
        Some(
            &[
                ("DynamicWorld", &dynamic_world),
                ("_from_world", &from_world),
                ("_serialize", &serialize),
            ]
            .into_py_dict(py)?,
        ),
        None,
    )
}
