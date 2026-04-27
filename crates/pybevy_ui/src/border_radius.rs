use bevy::ui::{BorderRadius, Val};
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyclass(name = "BorderRadius", frozen)]
#[derive(Clone, Debug)]
pub struct PyBorderRadius {
    pub(crate) inner: BorderRadius,
}

impl From<BorderRadius> for PyBorderRadius {
    fn from(br: BorderRadius) -> Self {
        Self { inner: br }
    }
}

impl From<PyBorderRadius> for BorderRadius {
    fn from(py_br: PyBorderRadius) -> Self {
        py_br.inner
    }
}

impl From<&PyBorderRadius> for BorderRadius {
    fn from(py_br: &PyBorderRadius) -> Self {
        py_br.inner
    }
}

#[pymethods]
impl PyBorderRadius {
    #[classattr]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self {
            inner: BorderRadius::ZERO,
        }
    }

    #[classattr]
    #[pyo3(name = "MAX")]
    pub fn max() -> Self {
        Self {
            inner: BorderRadius::MAX,
        }
    }

    #[new]
    #[pyo3(signature = (radius = PyVal::px(0.0), *, top_left = None, top_right = None, bottom_left = None, bottom_right = None))]
    pub fn py_new(
        radius: PyVal,
        top_left: Option<PyVal>,
        top_right: Option<PyVal>,
        bottom_left: Option<PyVal>,
        bottom_right: Option<PyVal>,
    ) -> Self {
        let base: Val = radius.into();
        Self {
            inner: BorderRadius {
                top_left: top_left.map(Into::into).unwrap_or(base),
                top_right: top_right.map(Into::into).unwrap_or(base),
                bottom_left: bottom_left.map(Into::into).unwrap_or(base),
                bottom_right: bottom_right.map(Into::into).unwrap_or(base),
            },
        }
    }

    #[staticmethod]
    #[pyo3(name = "new")]
    pub fn new_static(
        top_left: PyVal,
        top_right: PyVal,
        bottom_right: PyVal,
        bottom_left: PyVal,
    ) -> Self {
        Self {
            inner: BorderRadius::new(
                top_left.into(),
                top_right.into(),
                bottom_right.into(),
                bottom_left.into(),
            ),
        }
    }

    #[staticmethod]
    pub fn all(radius: PyVal) -> Self {
        Self {
            inner: BorderRadius::all(radius.into()),
        }
    }

    #[staticmethod]
    pub fn px(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            inner: BorderRadius::px(top_left, top_right, bottom_right, bottom_left),
        }
    }

    #[staticmethod]
    pub fn percent(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            inner: BorderRadius::percent(top_left, top_right, bottom_right, bottom_left),
        }
    }

    #[getter]
    pub fn get_top_left(&self) -> PyVal {
        self.inner.top_left.into()
    }

    #[getter]
    pub fn get_top_right(&self) -> PyVal {
        self.inner.top_right.into()
    }

    #[getter]
    pub fn get_bottom_left(&self) -> PyVal {
        self.inner.bottom_left.into()
    }

    #[getter]
    pub fn get_bottom_right(&self) -> PyVal {
        self.inner.bottom_right.into()
    }

    // Builder methods
    pub fn with_top_left(&self, radius: PyVal) -> Self {
        self.inner.with_top_left(radius.into()).into()
    }

    pub fn with_top_right(&self, radius: PyVal) -> Self {
        self.inner.with_top_right(radius.into()).into()
    }

    pub fn with_bottom_right(&self, radius: PyVal) -> Self {
        self.inner.with_bottom_right(radius.into()).into()
    }

    pub fn with_bottom_left(&self, radius: PyVal) -> Self {
        self.inner.with_bottom_left(radius.into()).into()
    }

    pub fn with_left(&self, radius: PyVal) -> Self {
        self.inner.with_left(radius.into()).into()
    }

    pub fn with_right(&self, radius: PyVal) -> Self {
        self.inner.with_right(radius.into()).into()
    }

    pub fn with_top(&self, radius: PyVal) -> Self {
        self.inner.with_top(radius.into()).into()
    }

    pub fn with_bottom(&self, radius: PyVal) -> Self {
        self.inner.with_bottom(radius.into()).into()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "BorderRadius(top_left={:?}, top_right={:?}, bottom_left={:?}, bottom_right={:?})",
            self.inner.top_left,
            self.inner.top_right,
            self.inner.bottom_left,
            self.inner.bottom_right
        )
    }
}
