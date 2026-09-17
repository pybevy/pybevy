use bevy::{ecs::entity::Entity, window::MonitorSelection};
use pybevy_core::PyEntity;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MonitorSelection, empty_tuple, no_repr)]
#[pyclass(
    name = "MonitorSelection",
    module = "pybevy.window",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyMonitorSelection {
    Current(),
    Primary(),
    #[py_bevy(tuple)]
    Index {
        index: usize,
    },
    #[py_bevy(tuple)]
    Entity {
        #[py_type(PyEntity)]
        entity: Entity,
    },
}

#[pymethods]
impl PyMonitorSelection {
    pub fn __repr__(&self) -> String {
        match self {
            PyMonitorSelection::Current() => "MonitorSelection.Current()".to_string(),
            PyMonitorSelection::Primary() => "MonitorSelection.Primary()".to_string(),
            PyMonitorSelection::Index { index } => format!("MonitorSelection.Index({index})"),
            PyMonitorSelection::Entity { entity } => {
                format!("MonitorSelection.Entity({:?})", entity)
            }
        }
    }
}
