use bevy::{
    math::IVec2,
    window::{MonitorSelection, WindowPosition},
};
use pyo3::prelude::*;

use crate::monitor_selection::PyMonitorSelection;

#[pyclass(name = "WindowPosition", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowPosition(pub(crate) WindowPosition);

impl From<WindowPosition> for PyWindowPosition {
    fn from(value: WindowPosition) -> Self {
        PyWindowPosition(value)
    }
}

impl From<PyWindowPosition> for WindowPosition {
    fn from(value: PyWindowPosition) -> Self {
        value.0
    }
}

#[pymethods]
impl PyWindowPosition {
    #[new]
    #[pyo3(signature = (x=None, y=None))]
    pub fn new(x: Option<i32>, y: Option<i32>) -> Self {
        match (x, y) {
            (Some(x), Some(y)) => PyWindowPosition(WindowPosition::At(IVec2::new(x, y))),
            _ => PyWindowPosition(WindowPosition::Automatic),
        }
    }

    #[staticmethod]
    pub fn center(monitor: PyMonitorSelection) -> Self {
        PyWindowPosition(WindowPosition::Centered(MonitorSelection::from(monitor)))
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            WindowPosition::Automatic => "WindowPosition()".to_string(),
            WindowPosition::Centered(monitor) => {
                let monitor_repr = match monitor {
                    MonitorSelection::Current => "MonitorSelection.Current()".to_string(),
                    MonitorSelection::Primary => "MonitorSelection.Primary()".to_string(),
                    MonitorSelection::Index(i) => format!("MonitorSelection.Index({})", i),
                    MonitorSelection::Entity(_) => "MonitorSelection.Entity(...)".to_string(),
                };
                format!("WindowPosition.center({})", monitor_repr)
            }
            WindowPosition::At(pos) => {
                format!("WindowPosition({}, {})", pos.x, pos.y)
            }
        }
    }
}
