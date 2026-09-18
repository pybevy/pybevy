use bevy::ui::UiPosition;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyclass(name = "UiPosition", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
#[pyvalue]
pub struct PyUiPosition {
    pub(crate) storage: ValueStorage<UiPosition>,
}

impl From<UiPosition> for PyUiPosition {
    fn from(pos: UiPosition) -> Self {
        PyUiPosition::from_owned(pos)
    }
}

impl TryFrom<PyUiPosition> for UiPosition {
    type Error = PyErr;

    fn try_from(py_pos: PyUiPosition) -> PyResult<Self> {
        py_pos.to_bevy()
    }
}

impl TryFrom<&PyUiPosition> for UiPosition {
    type Error = PyErr;

    fn try_from(py_pos: &PyUiPosition) -> PyResult<Self> {
        py_pos.to_bevy()
    }
}

#[pymethods]
impl PyUiPosition {
    #[new]
    #[pyo3(signature = (anchor, x, y))]
    pub fn new(anchor: PyVec2, x: PyVal, y: PyVal) -> PyResult<Self> {
        Ok(Self::from_owned(UiPosition::new(
            anchor.try_into()?,
            x.into(),
            y.into(),
        )))
    }

    #[staticmethod]
    pub fn anchor(anchor: PyVec2) -> PyResult<Self> {
        Ok(Self::from_owned(UiPosition::anchor(anchor.try_into()?)))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn center(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::center(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn top(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::top(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn bottom(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::bottom(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn left(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::left(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn right(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::right(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn top_left(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::top_left(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn top_right(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::top_right(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn bottom_left(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::bottom_left(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(signature = (x=PyVal::default(), y=PyVal::default()))]
    pub fn bottom_right(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(UiPosition::bottom_right(x.into(), y.into()))
    }

    pub fn at(&self, x: PyVal, y: PyVal) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()?.at(x.into(), y.into())))
    }

    pub fn at_x(&self, x: PyVal) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()?.at_x(x.into())))
    }

    pub fn at_y(&self, y: PyVal) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()?.at_y(y.into())))
    }

    pub fn at_px(&self, x: f32, y: f32) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()?.at_px(x, y)))
    }

    pub fn at_percent(&self, x: f32, y: f32) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()?.at_percent(x, y)))
    }

    #[getter]
    pub fn anchor_value(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_borrowed(ValueStorage::read_only_snapshot(
            self.to_bevy()?.anchor,
        )))
    }

    #[setter]
    pub fn set_anchor_value(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.anchor = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn x(&self) -> PyResult<PyVal> {
        Ok(self.to_bevy()?.x.into())
    }

    #[setter]
    pub fn set_x(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.x = value.into();
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<PyVal> {
        Ok(self.to_bevy()?.y.into())
    }

    #[setter]
    pub fn set_y(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.y = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let position = self.to_bevy()?;
        Ok(format!(
            "UiPosition(anchor={:?}, x={:?}, y={:?})",
            position.anchor, position.x, position.y
        ))
    }
}
