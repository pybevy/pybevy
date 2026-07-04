use bevy::ui::UiPosition;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyclass(name = "UiPosition", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyUiPosition {
    pub(crate) inner: UiPosition,
}

impl From<UiPosition> for PyUiPosition {
    fn from(pos: UiPosition) -> Self {
        PyUiPosition { inner: pos }
    }
}

impl From<PyUiPosition> for UiPosition {
    fn from(py_pos: PyUiPosition) -> Self {
        py_pos.inner
    }
}

#[pymethods]
impl PyUiPosition {
    #[new]
    #[pyo3(signature = (anchor, x, y))]
    pub fn new(anchor: PyVec2, x: PyVal, y: PyVal) -> Self {
        PyUiPosition {
            inner: UiPosition::new(anchor.into(), x.into(), y.into()),
        }
    }

    #[staticmethod]
    pub fn anchor(anchor: PyVec2) -> Self {
        PyUiPosition {
            inner: UiPosition::anchor(anchor.into()),
        }
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn center(x: PyVal, y: PyVal) -> Self {
        UiPosition::center(x.into(), y.into()).into()
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn top(x: PyVal, y: PyVal) -> Self {
        UiPosition::top(x.into(), y.into()).into()
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn bottom(x: PyVal, y: PyVal) -> Self {
        UiPosition::bottom(x.into(), y.into()).into()
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn left(x: PyVal, y: PyVal) -> Self {
        UiPosition::left(x.into(), y.into()).into()
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn right(x: PyVal, y: PyVal) -> Self {
        UiPosition::right(x.into(), y.into()).into()
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn top_left(x: PyVal, y: PyVal) -> Self {
        UiPosition::top_left(x.into(), y.into()).into()
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn top_right(x: PyVal, y: PyVal) -> Self {
        UiPosition::top_right(x.into(), y.into()).into()
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn bottom_left(x: PyVal, y: PyVal) -> Self {
        UiPosition::bottom_left(x.into(), y.into()).into()
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::new(), y=PyVal::new()))]
    pub fn bottom_right(x: PyVal, y: PyVal) -> Self {
        UiPosition::bottom_right(x.into(), y.into()).into()
    }

    pub fn at(&self, x: PyVal, y: PyVal) -> Self {
        PyUiPosition {
            inner: self.inner.at(x.into(), y.into()),
        }
    }

    pub fn at_x(&self, x: PyVal) -> Self {
        PyUiPosition {
            inner: self.inner.at_x(x.into()),
        }
    }

    pub fn at_y(&self, y: PyVal) -> Self {
        PyUiPosition {
            inner: self.inner.at_y(y.into()),
        }
    }

    pub fn at_px(&self, x: f32, y: f32) -> Self {
        PyUiPosition {
            inner: self.inner.at_px(x, y),
        }
    }

    pub fn at_percent(&self, x: f32, y: f32) -> Self {
        PyUiPosition {
            inner: self.inner.at_percent(x, y),
        }
    }

    #[getter]
    pub fn anchor_value(&self) -> PyVec2 {
        self.inner.anchor.into()
    }

    #[getter]
    pub fn x(&self) -> PyVal {
        self.inner.x.into()
    }

    #[getter]
    pub fn y(&self) -> PyVal {
        self.inner.y.into()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "UiPosition(anchor={:?}, x={:?}, y={:?})",
            self.inner.anchor, self.inner.x, self.inner.y
        )
    }
}
