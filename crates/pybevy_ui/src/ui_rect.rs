use bevy::ui::UiRect;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyclass(name = "UiRect", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyUiRect {
    pub(crate) inner: UiRect,
}

impl From<UiRect> for PyUiRect {
    fn from(rect: UiRect) -> Self {
        PyUiRect { inner: rect }
    }
}

impl From<PyUiRect> for UiRect {
    fn from(py_rect: PyUiRect) -> Self {
        py_rect.inner
    }
}

#[pymethods]
impl PyUiRect {
    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        PyUiRect {
            inner: UiRect::ZERO,
        }
    }

    #[staticmethod]
    #[pyo3(name = "AUTO")]
    pub fn auto_() -> Self {
        PyUiRect {
            inner: UiRect::AUTO,
        }
    }

    #[staticmethod]
    #[pyo3(name = "DEFAULT")]
    pub fn default_() -> Self {
        PyUiRect {
            inner: UiRect::DEFAULT,
        }
    }

    #[new]
    #[pyo3(signature = (left = PyVal::px(0.0), right = PyVal::px(0.0), top = PyVal::px(0.0), bottom = PyVal::px(0.0)))]
    pub fn py_new(left: PyVal, right: PyVal, top: PyVal, bottom: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::new(left.into(), right.into(), top.into(), bottom.into()),
        }
    }

    #[staticmethod]
    #[pyo3(name = "new")]
    pub fn new_(left: PyVal, right: PyVal, top: PyVal, bottom: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::new(left.into(), right.into(), top.into(), bottom.into()),
        }
    }

    #[staticmethod]
    pub fn all(value: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::all(value.into()),
        }
    }

    #[staticmethod]
    pub fn px(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        PyUiRect {
            inner: UiRect::px(left, right, top, bottom),
        }
    }

    #[staticmethod]
    pub fn percent(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        PyUiRect {
            inner: UiRect::percent(left, right, top, bottom),
        }
    }

    #[staticmethod]
    pub fn horizontal(value: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::horizontal(value.into()),
        }
    }

    #[staticmethod]
    pub fn vertical(value: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::vertical(value.into()),
        }
    }

    #[staticmethod]
    pub fn axes(horizontal: PyVal, vertical: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::axes(horizontal.into(), vertical.into()),
        }
    }

    #[staticmethod]
    pub fn left(left: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::left(left.into()),
        }
    }

    #[staticmethod]
    pub fn right(right: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::right(right.into()),
        }
    }

    #[staticmethod]
    pub fn top(top: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::top(top.into()),
        }
    }

    #[staticmethod]
    pub fn bottom(bottom: PyVal) -> Self {
        PyUiRect {
            inner: UiRect::bottom(bottom.into()),
        }
    }

    pub fn with_left(&self, left: PyVal) -> Self {
        PyUiRect {
            inner: self.inner.with_left(left.into()),
        }
    }

    pub fn with_right(&self, right: PyVal) -> Self {
        PyUiRect {
            inner: self.inner.with_right(right.into()),
        }
    }

    pub fn with_top(&self, top: PyVal) -> Self {
        PyUiRect {
            inner: self.inner.with_top(top.into()),
        }
    }

    pub fn with_bottom(&self, bottom: PyVal) -> Self {
        PyUiRect {
            inner: self.inner.with_bottom(bottom.into()),
        }
    }

    pub fn get_left(&self) -> PyVal {
        self.inner.left.into()
    }

    pub fn set_left(&mut self, value: PyVal) {
        self.inner.left = value.into();
    }

    pub fn get_right(&self) -> PyVal {
        self.inner.right.into()
    }

    pub fn set_right(&mut self, value: PyVal) {
        self.inner.right = value.into();
    }

    pub fn get_top(&self) -> PyVal {
        self.inner.top.into()
    }

    pub fn set_top(&mut self, value: PyVal) {
        self.inner.top = value.into();
    }

    pub fn get_bottom(&self) -> PyVal {
        self.inner.bottom.into()
    }

    pub fn set_bottom(&mut self, value: PyVal) {
        self.inner.bottom = value.into();
    }

    pub fn __repr__(&self) -> String {
        format!(
            "UiRect(left={}, right={}, top={}, bottom={})",
            PyVal::from(self.inner.left).__repr__(),
            PyVal::from(self.inner.right).__repr__(),
            PyVal::from(self.inner.top).__repr__(),
            PyVal::from(self.inner.bottom).__repr__()
        )
    }
}
