use bevy::window::MonitorSelection;
use pybevy_core::PyEntity;
use pyo3::prelude::*;

#[pyclass(name = "MonitorSelection", eq, frozen, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyMonitorSelection {
    Current(),
    Primary(),
    Index(usize),
    Entity(PyEntity),
}

impl From<PyMonitorSelection> for MonitorSelection {
    fn from(value: PyMonitorSelection) -> Self {
        match value {
            PyMonitorSelection::Current() => MonitorSelection::Current,
            PyMonitorSelection::Primary() => MonitorSelection::Primary,
            PyMonitorSelection::Index(idx) => MonitorSelection::Index(idx),
            PyMonitorSelection::Entity(entity) => MonitorSelection::Entity(entity.0),
        }
    }
}

impl From<MonitorSelection> for PyMonitorSelection {
    fn from(value: MonitorSelection) -> Self {
        match value {
            MonitorSelection::Current => PyMonitorSelection::Current(),
            MonitorSelection::Primary => PyMonitorSelection::Primary(),
            MonitorSelection::Index(idx) => PyMonitorSelection::Index(idx),
            MonitorSelection::Entity(entity) => PyMonitorSelection::Entity(PyEntity(entity)),
        }
    }
}

#[pymethods]
impl PyMonitorSelection {
    fn __repr__(&self) -> String {
        match self {
            PyMonitorSelection::Current() => "MonitorSelection.Current()".to_string(),
            PyMonitorSelection::Primary() => "MonitorSelection.Primary()".to_string(),
            PyMonitorSelection::Index(idx) => format!("MonitorSelection.Index({idx})"),
            PyMonitorSelection::Entity(entity) => {
                format!("MonitorSelection.Entity({:?})", entity)
            }
        }
    }
}
