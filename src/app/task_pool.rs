use bevy::app::TaskPoolPlugin;
use pybevy_core::PyPlugin;
use pyo3::prelude::*;

use crate::app::app::PyApp;

#[pyclass(name = "TaskPoolPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyTaskPoolPlugin;

#[pymethods]
impl PyTaskPoolPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyTaskPoolPlugin, PyPlugin).into()
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(TaskPoolPlugin::default());
            Ok(())
        })
    }
}
