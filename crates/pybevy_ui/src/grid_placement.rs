use bevy::ui::GridPlacement;
use pybevy_core::{
    domain::{IntegerInput, NON_ZERO, POSITIVE, check_for},
    repr::ReprLiteral,
};
use pyo3::prelude::*;

#[pyclass(
    name = "GridPlacement",
    module = "pybevy.ui",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyGridPlacement {
    pub(crate) inner: GridPlacement,
}

impl From<GridPlacement> for PyGridPlacement {
    fn from(placement: GridPlacement) -> Self {
        PyGridPlacement { inner: placement }
    }
}

impl From<PyGridPlacement> for GridPlacement {
    fn from(py_placement: PyGridPlacement) -> Self {
        py_placement.inner
    }
}

impl Default for PyGridPlacement {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PyGridPlacement {
    #[new]
    pub fn new() -> Self {
        PyGridPlacement {
            inner: GridPlacement::default(),
        }
    }

    #[staticmethod]
    pub fn auto() -> Self {
        PyGridPlacement {
            inner: GridPlacement::default(),
        }
    }

    #[staticmethod]
    pub fn span(span: IntegerInput<u16>) -> PyResult<Self> {
        Ok(GridPlacement::span(check_for::<Self, _>("span", span, POSITIVE)?.stored()).into())
    }

    #[staticmethod]
    pub fn start(start: IntegerInput<i16>) -> PyResult<Self> {
        Ok(GridPlacement::start(check_for::<Self, _>("start", start, NON_ZERO)?.stored()).into())
    }

    #[staticmethod]
    pub fn end(end: IntegerInput<i16>) -> PyResult<Self> {
        Ok(GridPlacement::end(check_for::<Self, _>("end", end, NON_ZERO)?.stored()).into())
    }

    #[staticmethod]
    pub fn start_span(start: IntegerInput<i16>, span: IntegerInput<u16>) -> PyResult<Self> {
        Ok(GridPlacement::start_span(
            check_for::<Self, _>("start", start, NON_ZERO)?.stored(),
            check_for::<Self, _>("span", span, POSITIVE)?.stored(),
        )
        .into())
    }

    #[staticmethod]
    pub fn start_end(start: IntegerInput<i16>, end: IntegerInput<i16>) -> PyResult<Self> {
        Ok(GridPlacement::start_end(
            check_for::<Self, _>("start", start, NON_ZERO)?.stored(),
            check_for::<Self, _>("end", end, NON_ZERO)?.stored(),
        )
        .into())
    }

    #[staticmethod]
    pub fn end_span(end: IntegerInput<i16>, span: IntegerInput<u16>) -> PyResult<Self> {
        Ok(GridPlacement::end_span(
            check_for::<Self, _>("end", end, NON_ZERO)?.stored(),
            check_for::<Self, _>("span", span, POSITIVE)?.stored(),
        )
        .into())
    }

    pub fn get_start(&self) -> Option<i16> {
        self.inner.get_start()
    }

    pub fn get_span(&self) -> Option<u16> {
        self.inner.get_span()
    }

    pub fn get_end(&self) -> Option<i16> {
        self.inner.get_end()
    }

    pub fn set_start(&self, start: IntegerInput<i16>) -> PyResult<Self> {
        Ok(self
            .inner
            .set_start(check_for::<Self, _>("start", start, NON_ZERO)?.stored())
            .into())
    }

    pub fn set_end(&self, end: IntegerInput<i16>) -> PyResult<Self> {
        Ok(self
            .inner
            .set_end(check_for::<Self, _>("end", end, NON_ZERO)?.stored())
            .into())
    }

    pub fn set_span(&self, span: IntegerInput<u16>) -> PyResult<Self> {
        Ok(self
            .inner
            .set_span(check_for::<Self, _>("span", span, POSITIVE)?.stored())
            .into())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "GridPlacement(start={}, span={}, end={})",
            self.inner.get_start().repr_literal(),
            self.inner.get_span().repr_literal(),
            self.inner.get_end().repr_literal()
        )
    }
}
