use bevy::{math::Vec2, window::CursorMoved};
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::pymessage;
use pybevy_math::snapshot::Snapshot;
use pyo3::prelude::*;

#[pymessage(CursorMoved, writable)]
#[pyclass(name = "CursorMoved", module = "pybevy.window", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCursorMoved {
    pub position: Vec2,
    pub delta: Option<Vec2>,
    pub window: PyEntity,
}

impl From<&CursorMoved> for PyCursorMoved {
    fn from(event: &CursorMoved) -> Self {
        PyCursorMoved {
            position: event.position,
            delta: event.delta,
            window: event.window.into(),
        }
    }
}

impl TryFrom<&PyCursorMoved> for CursorMoved {
    type Error = PyErr;

    fn try_from(value: &PyCursorMoved) -> PyResult<Self> {
        Ok(CursorMoved {
            window: value.window.into(),
            position: value.position,
            delta: value.delta,
        })
    }
}

#[pymethods]
impl PyCursorMoved {
    #[new]
    #[pyo3(signature = (*, window, position, delta=None))]
    fn new(
        window: PyEntity,
        position: Snapshot<Vec2>,
        delta: Option<Snapshot<Vec2>>,
    ) -> PyClassInitializer<Self> {
        (
            PyCursorMoved {
                position: position.0,
                delta: delta.map(|delta| delta.0),
                window,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn position(&self) -> Snapshot<Vec2> {
        Snapshot(self.position)
    }

    #[setter]
    fn set_position(&mut self, position: Snapshot<Vec2>) {
        self.position = position.0;
    }

    #[getter]
    fn delta(&self) -> Option<Snapshot<Vec2>> {
        self.delta.map(Snapshot)
    }

    #[setter]
    fn set_delta(&mut self, delta: Option<Snapshot<Vec2>>) {
        self.delta = delta.map(|delta| delta.0);
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    #[setter]
    fn set_window(&mut self, window: PyEntity) {
        self.window = window;
    }

    fn __repr__(&self) -> String {
        let delta = self
            .delta
            .map(|delta| format!("Vec2({}, {})", delta.x, delta.y));
        format!(
            "CursorMoved(position=Vec2({}, {}), delta={:?})",
            self.position.x, self.position.y, delta
        )
    }
}
