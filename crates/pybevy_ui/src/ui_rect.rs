use bevy::ui::UiRect;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyvalue]
#[pyclass(name = "UiRect", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyUiRect {
    pub(crate) storage: ValueStorage<UiRect>,
}

impl PartialEq for PyUiRect {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl From<UiRect> for PyUiRect {
    fn from(rect: UiRect) -> Self {
        Self::from_owned(rect)
    }
}

impl TryFrom<PyUiRect> for UiRect {
    type Error = PyErr;

    fn try_from(py_rect: PyUiRect) -> PyResult<Self> {
        py_rect.to_bevy()
    }
}

impl TryFrom<&PyUiRect> for UiRect {
    type Error = PyErr;

    fn try_from(py_rect: &PyUiRect) -> PyResult<Self> {
        py_rect.to_bevy()
    }
}

#[pymethods]
impl PyUiRect {
    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::from_owned(UiRect::ZERO)
    }

    #[staticmethod]
    #[pyo3(name = "AUTO")]
    pub fn auto_() -> Self {
        Self::from_owned(UiRect::AUTO)
    }

    #[staticmethod]
    #[pyo3(name = "DEFAULT")]
    pub fn default_() -> Self {
        Self::from_owned(UiRect::DEFAULT)
    }

    #[new]
    #[pyo3(signature = (left = PyVal::px_unchecked(0.0), right = PyVal::px_unchecked(0.0), top = PyVal::px_unchecked(0.0), bottom = PyVal::px_unchecked(0.0)))]
    pub fn py_new(left: PyVal, right: PyVal, top: PyVal, bottom: PyVal) -> Self {
        Self::from_owned(UiRect::new(
            left.into(),
            right.into(),
            top.into(),
            bottom.into(),
        ))
    }

    #[staticmethod]
    #[pyo3(name = "new")]
    pub fn new_(left: PyVal, right: PyVal, top: PyVal, bottom: PyVal) -> Self {
        Self::from_owned(UiRect::new(
            left.into(),
            right.into(),
            top.into(),
            bottom.into(),
        ))
    }

    #[staticmethod]
    pub fn all(value: PyVal) -> Self {
        Self::from_owned(UiRect::all(value.into()))
    }

    #[staticmethod]
    pub fn px(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self::from_owned(UiRect::px(left, right, top, bottom))
    }

    #[staticmethod]
    pub fn percent(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self::from_owned(UiRect::percent(left, right, top, bottom))
    }

    #[staticmethod]
    pub fn horizontal(value: PyVal) -> Self {
        Self::from_owned(UiRect::horizontal(value.into()))
    }

    #[staticmethod]
    pub fn vertical(value: PyVal) -> Self {
        Self::from_owned(UiRect::vertical(value.into()))
    }

    #[staticmethod]
    pub fn axes(horizontal: PyVal, vertical: PyVal) -> Self {
        Self::from_owned(UiRect::axes(horizontal.into(), vertical.into()))
    }

    #[staticmethod]
    pub fn left(left: PyVal) -> Self {
        Self::from_owned(UiRect::left(left.into()))
    }

    #[staticmethod]
    pub fn right(right: PyVal) -> Self {
        Self::from_owned(UiRect::right(right.into()))
    }

    #[staticmethod]
    pub fn top(top: PyVal) -> Self {
        Self::from_owned(UiRect::top(top.into()))
    }

    #[staticmethod]
    pub fn bottom(bottom: PyVal) -> Self {
        Self::from_owned(UiRect::bottom(bottom.into()))
    }

    pub fn with_left(&self, left: PyVal) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_left(left.into())))
    }

    pub fn with_right(&self, right: PyVal) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_right(right.into())))
    }

    pub fn with_top(&self, top: PyVal) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_top(top.into())))
    }

    pub fn with_bottom(&self, bottom: PyVal) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_bottom(bottom.into())))
    }

    pub fn get_left(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.left.into())
    }

    pub fn set_left(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.left = value.into();
        Ok(())
    }

    pub fn get_right(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.right.into())
    }

    pub fn set_right(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.right = value.into();
        Ok(())
    }

    pub fn get_top(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.top.into())
    }

    pub fn set_top(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.top = value.into();
        Ok(())
    }

    pub fn get_bottom(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.bottom.into())
    }

    pub fn set_bottom(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.bottom = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let rect = self.as_ref()?;
        Ok(format!(
            "UiRect(left={}, right={}, top={}, bottom={})",
            PyVal::from(rect.left).__repr__(),
            PyVal::from(rect.right).__repr__(),
            PyVal::from(rect.top).__repr__(),
            PyVal::from(rect.bottom).__repr__()
        ))
    }
}
