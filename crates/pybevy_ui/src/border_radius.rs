use bevy::ui::{BorderRadius, Val};
use pybevy_core::{FromBorrowedStorage, Omittable, ValueStorage, repr::object_repr};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyvalue]
#[pyclass(name = "BorderRadius", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyBorderRadius {
    pub(crate) storage: ValueStorage<BorderRadius>,
}

impl PartialEq for PyBorderRadius {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl From<BorderRadius> for PyBorderRadius {
    fn from(br: BorderRadius) -> Self {
        Self::from_owned(br)
    }
}

impl TryFrom<PyBorderRadius> for BorderRadius {
    type Error = PyErr;

    fn try_from(py_br: PyBorderRadius) -> PyResult<Self> {
        py_br.to_bevy()
    }
}

impl TryFrom<&PyBorderRadius> for BorderRadius {
    type Error = PyErr;

    fn try_from(py_br: &PyBorderRadius) -> PyResult<Self> {
        py_br.to_bevy()
    }
}

#[pymethods]
impl PyBorderRadius {
    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::from_owned(BorderRadius::ZERO)
    }

    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max() -> Self {
        Self::from_owned(BorderRadius::MAX)
    }

    #[new]
    #[pyo3(signature = (radius = PyVal::px_unchecked(0.0), *, top_left = Omittable::default(), top_right = Omittable::default(), bottom_right = Omittable::default(), bottom_left = Omittable::default()))]
    pub fn py_new(
        radius: PyVal,
        top_left: Omittable<PyRef<'_, PyVal>>,
        top_right: Omittable<PyRef<'_, PyVal>>,
        bottom_right: Omittable<PyRef<'_, PyVal>>,
        bottom_left: Omittable<PyRef<'_, PyVal>>,
    ) -> Self {
        let base: Val = radius.into();
        Self::from_owned(BorderRadius {
            top_left: top_left
                .into_option()
                .map_or(base, |value| value.clone().into()),
            top_right: top_right
                .into_option()
                .map_or(base, |value| value.clone().into()),
            bottom_right: bottom_right
                .into_option()
                .map_or(base, |value| value.clone().into()),
            bottom_left: bottom_left
                .into_option()
                .map_or(base, |value| value.clone().into()),
        })
    }

    #[staticmethod]
    #[pyo3(name = "new")]
    pub fn new_static(
        top_left: PyVal,
        top_right: PyVal,
        bottom_right: PyVal,
        bottom_left: PyVal,
    ) -> Self {
        Self::from_owned(BorderRadius::new(
            top_left.into(),
            top_right.into(),
            bottom_right.into(),
            bottom_left.into(),
        ))
    }

    #[staticmethod]
    pub fn left(radius: PyVal) -> Self {
        Self::from_owned(BorderRadius::left(radius.into()))
    }

    #[staticmethod]
    pub fn right(radius: PyVal) -> Self {
        Self::from_owned(BorderRadius::right(radius.into()))
    }

    #[staticmethod]
    pub fn top(radius: PyVal) -> Self {
        Self::from_owned(BorderRadius::top(radius.into()))
    }

    #[staticmethod]
    pub fn bottom(radius: PyVal) -> Self {
        Self::from_owned(BorderRadius::bottom(radius.into()))
    }

    #[staticmethod]
    pub fn all(radius: PyVal) -> Self {
        Self::from_owned(BorderRadius::all(radius.into()))
    }

    #[staticmethod]
    pub fn px(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self::from_owned(BorderRadius::px(
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        ))
    }

    #[staticmethod]
    pub fn percent(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self::from_owned(BorderRadius::percent(
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        ))
    }

    #[getter]
    pub fn get_top_left(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.top_left.into())
    }

    #[setter]
    pub fn set_top_left(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.top_left = value.into();
        Ok(())
    }

    #[getter]
    pub fn get_top_right(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.top_right.into())
    }

    #[setter]
    pub fn set_top_right(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.top_right = value.into();
        Ok(())
    }

    #[getter]
    pub fn get_bottom_left(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.bottom_left.into())
    }

    #[setter]
    pub fn set_bottom_left(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.bottom_left = value.into();
        Ok(())
    }

    #[getter]
    pub fn get_bottom_right(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.bottom_right.into())
    }

    #[setter]
    pub fn set_bottom_right(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.bottom_right = value.into();
        Ok(())
    }

    // Builder methods
    pub fn with_top_left(&self, radius: PyVal) -> PyResult<Self> {
        Ok(self.as_ref()?.with_top_left(radius.into()).into())
    }

    pub fn with_top_right(&self, radius: PyVal) -> PyResult<Self> {
        Ok(self.as_ref()?.with_top_right(radius.into()).into())
    }

    pub fn with_bottom_right(&self, radius: PyVal) -> PyResult<Self> {
        Ok(self.as_ref()?.with_bottom_right(radius.into()).into())
    }

    pub fn with_bottom_left(&self, radius: PyVal) -> PyResult<Self> {
        Ok(self.as_ref()?.with_bottom_left(radius.into()).into())
    }

    pub fn with_left(&self, radius: PyVal) -> PyResult<Self> {
        Ok(self.as_ref()?.with_left(radius.into()).into())
    }

    pub fn with_right(&self, radius: PyVal) -> PyResult<Self> {
        Ok(self.as_ref()?.with_right(radius.into()).into())
    }

    pub fn with_top(&self, radius: PyVal) -> PyResult<Self> {
        Ok(self.as_ref()?.with_top(radius.into()).into())
    }

    pub fn with_bottom(&self, radius: PyVal) -> PyResult<Self> {
        Ok(self.as_ref()?.with_bottom(radius.into()).into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let radius = self.as_ref()?;
        Ok(format!(
            "BorderRadius(top_left={}, top_right={}, bottom_right={}, bottom_left={})",
            object_repr(PyVal::from(radius.top_left)),
            object_repr(PyVal::from(radius.top_right)),
            object_repr(PyVal::from(radius.bottom_right)),
            object_repr(PyVal::from(radius.bottom_left))
        ))
    }
}
