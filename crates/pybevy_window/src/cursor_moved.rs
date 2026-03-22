use bevy::window::CursorMoved;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::message_bridge;
use pybevy_math::PyVec2;
use pyo3::prelude::*;

#[pyclass(name = "CursorMoved", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCursorMoved {
    pub position: PyVec2,
    pub delta: Option<PyVec2>,
    pub window: PyEntity,
}

impl From<&CursorMoved> for PyCursorMoved {
    fn from(event: &CursorMoved) -> Self {
        PyCursorMoved {
            position: event.position.into(),
            delta: event.delta.map(Into::into),
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyCursorMoved {
    #[new]
    #[pyo3(signature = (position, window, delta=None))]
    fn new(position: PyVec2, window: PyEntity, delta: Option<PyVec2>) -> (Self, PyMessage) {
        (
            PyCursorMoved {
                position,
                delta,
                window,
            },
            PyMessage,
        )
    }

    #[getter]
    fn position(&self) -> PyVec2 {
        self.position.clone()
    }

    #[getter]
    fn delta(&self) -> Option<PyVec2> {
        self.delta.clone()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        let pos = self.position.get();
        format!(
            "CursorMoved(position=Vec2({}, {}), delta={:?})",
            pos.x,
            pos.y,
            self.delta.as_ref().map(|d| {
                let dv = d.get();
                format!("Vec2({}, {})", dv.x, dv.y)
            })
        )
    }
}

message_bridge!(CursorMoved, PyCursorMoved);
