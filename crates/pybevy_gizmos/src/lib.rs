pub mod plugin;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::plugin::PyGizmoPlugin;
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "gizmos")?;
    m.add_class::<plugin::PyGizmoPlugin>()?;
    parent.add_submodule(&m)
}
