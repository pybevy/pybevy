pub mod aabb;
pub mod config;
pub mod gizmos;
pub mod plugin;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        aabb::PyShowAabbGizmo,
        config::{
            PyGizmoConfig, PyGizmoConfigGroup, PyGizmoConfigStore, PyGizmoLineConfig,
            PyGizmoLineJoint, PyGizmoLineStyle,
        },
        gizmos::PyGizmos,
        plugin::PyGizmoPlugin,
    };
}

pub use config::GizmoConfigGroupRegistration;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "gizmos")?;
    m.add_class::<aabb::PyShowAabbGizmo>()?;
    m.add_class::<config::PyGizmoLineJoint>()?;
    m.add_class::<config::PyGizmoLineStyle>()?;
    m.add_class::<config::PyGizmoLineConfig>()?;
    m.add_class::<config::PyGizmoConfig>()?;
    m.add_class::<config::PyGizmoConfigGroup>()?;
    m.add_class::<config::PyGizmoConfigStore>()?;
    m.add_class::<gizmos::PyGizmos>()?;
    m.add_class::<plugin::PyGizmoPlugin>()?;
    parent.add_submodule(&m)
}
