pub mod plugin;
pub mod update_mode;
pub mod winit_settings;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        plugin::PyWinitPlugin, update_mode::PyUpdateMode, winit_settings::PyWinitSettings,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "winit")?;
    m.add_class::<plugin::PyWinitPlugin>()?;
    m.add_class::<update_mode::PyUpdateMode>()?;
    m.add_class::<winit_settings::PyWinitSettings>()?;
    parent.add_submodule(&m)
}
