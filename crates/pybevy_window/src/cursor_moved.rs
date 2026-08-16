use bevy::window::CursorMoved;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::pymessage;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pymessage(CursorMoved)]
#[pyclass(name = "CursorMoved", extends = PyMessage, eq, skip_from_py_object)]
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
    fn new(position: PyVec2, window: PyEntity, delta: Option<PyVec2>) -> PyClassInitializer<Self> {
        (
            PyCursorMoved {
                position,
                delta,
                window,
            },
            PyMessage,
        )
            .into()
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

    fn __repr__(&self) -> PyResult<String> {
        let pos = self.position.try_get()?;
        let delta = match self.delta.as_ref() {
            Some(d) => {
                let dv = d.try_get()?;
                Some(format!("Vec2({}, {})", dv.x, dv.y))
            }
            None => None,
        };
        Ok(format!(
            "CursorMoved(position=Vec2({}, {}), delta={:?})",
            pos.x, pos.y, delta
        ))
    }
}
