use bevy::ui::GridPlacement;
use pyo3::prelude::*;

#[pyclass(name = "GridPlacement", eq, frozen, from_py_object)]
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
    pub fn span(span: u16) -> Self {
        PyGridPlacement {
            inner: GridPlacement::span(span),
        }
    }

    #[staticmethod]
    pub fn start(start: i16) -> Self {
        PyGridPlacement {
            inner: GridPlacement::start(start),
        }
    }

    #[staticmethod]
    pub fn end(end: i16) -> Self {
        PyGridPlacement {
            inner: GridPlacement::end(end),
        }
    }

    #[staticmethod]
    pub fn start_span(start: i16, span: u16) -> Self {
        PyGridPlacement {
            inner: GridPlacement::start_span(start, span),
        }
    }

    #[staticmethod]
    pub fn start_end(start: i16, end: i16) -> Self {
        PyGridPlacement {
            inner: GridPlacement::start_end(start, end),
        }
    }

    #[staticmethod]
    pub fn end_span(end: i16, span: u16) -> Self {
        PyGridPlacement {
            inner: GridPlacement::end_span(end, span),
        }
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

    pub fn set_start(&self, start: i16) -> Self {
        self.inner.set_start(start).into()
    }

    pub fn set_end(&self, end: i16) -> Self {
        self.inner.set_end(end).into()
    }

    pub fn set_span(&self, span: u16) -> Self {
        self.inner.set_span(span).into()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "GridPlacement(start={:?}, span={:?}, end={:?})",
            self.inner.get_start(),
            self.inner.get_span(),
            self.inner.get_end()
        )
    }
}
