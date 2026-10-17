use bevy::ui::{BorderRadius, CornerRadius};
use pybevy_core::{FromBorrowedStorage, Omittable, ValueStorage, repr::object_repr};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::corner_radius::{IntoCornerRadius, PyCornerRadius, extract_corner_radius_from_any};

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

    #[staticmethod]
    #[pyo3(name = "MAX_ELLIPTICAL")]
    pub fn max_elliptical() -> Self {
        Self::from_owned(BorderRadius::MAX_ELLIPTICAL)
    }

    #[new]
    #[pyo3(signature = (radius = IntoCornerRadius(CornerRadius::ZERO), *, top_left = Omittable::default(), top_right = Omittable::default(), bottom_right = Omittable::default(), bottom_left = Omittable::default()))]
    pub fn py_new(
        radius: IntoCornerRadius,
        top_left: Omittable<Bound<'_, PyAny>>,
        top_right: Omittable<Bound<'_, PyAny>>,
        bottom_right: Omittable<Bound<'_, PyAny>>,
        bottom_left: Omittable<Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let corner = |value: Omittable<Bound<'_, PyAny>>| {
            value
                .into_option()
                .map_or(Ok(radius.0), |value| extract_corner_radius_from_any(&value))
        };
        Ok(Self::from_owned(BorderRadius {
            top_left: corner(top_left)?,
            top_right: corner(top_right)?,
            bottom_right: corner(bottom_right)?,
            bottom_left: corner(bottom_left)?,
        }))
    }

    #[staticmethod]
    #[pyo3(name = "new")]
    pub fn new_static(
        top_left: IntoCornerRadius,
        top_right: IntoCornerRadius,
        bottom_right: IntoCornerRadius,
        bottom_left: IntoCornerRadius,
    ) -> Self {
        Self::from_owned(BorderRadius::new(
            top_left.0,
            top_right.0,
            bottom_right.0,
            bottom_left.0,
        ))
    }

    #[staticmethod]
    pub fn left(radius: IntoCornerRadius) -> Self {
        Self::from_owned(BorderRadius::left(radius.0))
    }

    #[staticmethod]
    pub fn right(radius: IntoCornerRadius) -> Self {
        Self::from_owned(BorderRadius::right(radius.0))
    }

    #[staticmethod]
    pub fn top(radius: IntoCornerRadius) -> Self {
        Self::from_owned(BorderRadius::top(radius.0))
    }

    #[staticmethod]
    pub fn bottom(radius: IntoCornerRadius) -> Self {
        Self::from_owned(BorderRadius::bottom(radius.0))
    }

    #[staticmethod]
    pub fn all(radius: IntoCornerRadius) -> Self {
        Self::from_owned(BorderRadius::all(radius.0))
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
    pub fn top_left(&self) -> PyResult<PyCornerRadius> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|radius| &radius.top_left, |radius| &mut radius.top_left)?)
    }

    #[setter]
    pub fn set_top_left(&mut self, value: IntoCornerRadius) -> PyResult<()> {
        self.as_mut()?.top_left = value.0;
        Ok(())
    }

    #[getter]
    pub fn top_right(&self) -> PyResult<PyCornerRadius> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|radius| &radius.top_right, |radius| &mut radius.top_right)?)
    }

    #[setter]
    pub fn set_top_right(&mut self, value: IntoCornerRadius) -> PyResult<()> {
        self.as_mut()?.top_right = value.0;
        Ok(())
    }

    #[getter]
    pub fn bottom_right(&self) -> PyResult<PyCornerRadius> {
        Ok(self.storage.borrow_resolved_field_as(
            |radius| &radius.bottom_right,
            |radius| &mut radius.bottom_right,
        )?)
    }

    #[setter]
    pub fn set_bottom_right(&mut self, value: IntoCornerRadius) -> PyResult<()> {
        self.as_mut()?.bottom_right = value.0;
        Ok(())
    }

    #[getter]
    pub fn bottom_left(&self) -> PyResult<PyCornerRadius> {
        Ok(self.storage.borrow_resolved_field_as(
            |radius| &radius.bottom_left,
            |radius| &mut radius.bottom_left,
        )?)
    }

    #[setter]
    pub fn set_bottom_left(&mut self, value: IntoCornerRadius) -> PyResult<()> {
        self.as_mut()?.bottom_left = value.0;
        Ok(())
    }

    pub fn with_top_left(&self, radius: IntoCornerRadius) -> PyResult<Self> {
        Ok(self.as_ref()?.with_top_left(radius.0).into())
    }

    pub fn with_top_right(&self, radius: IntoCornerRadius) -> PyResult<Self> {
        Ok(self.as_ref()?.with_top_right(radius.0).into())
    }

    pub fn with_bottom_right(&self, radius: IntoCornerRadius) -> PyResult<Self> {
        Ok(self.as_ref()?.with_bottom_right(radius.0).into())
    }

    pub fn with_bottom_left(&self, radius: IntoCornerRadius) -> PyResult<Self> {
        Ok(self.as_ref()?.with_bottom_left(radius.0).into())
    }

    pub fn with_left(&self, radius: IntoCornerRadius) -> PyResult<Self> {
        Ok(self.as_ref()?.with_left(radius.0).into())
    }

    pub fn with_right(&self, radius: IntoCornerRadius) -> PyResult<Self> {
        Ok(self.as_ref()?.with_right(radius.0).into())
    }

    pub fn with_top(&self, radius: IntoCornerRadius) -> PyResult<Self> {
        Ok(self.as_ref()?.with_top(radius.0).into())
    }

    pub fn with_bottom(&self, radius: IntoCornerRadius) -> PyResult<Self> {
        Ok(self.as_ref()?.with_bottom(radius.0).into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let radius = self.as_ref()?;
        Ok(format!(
            "BorderRadius(top_left={}, top_right={}, bottom_right={}, bottom_left={})",
            object_repr(PyCornerRadius::from(radius.top_left)),
            object_repr(PyCornerRadius::from(radius.top_right)),
            object_repr(PyCornerRadius::from(radius.bottom_right)),
            object_repr(PyCornerRadius::from(radius.bottom_left))
        ))
    }
}
