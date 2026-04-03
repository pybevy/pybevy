use bevy::text::{Justify, LineBreak, TextLayout};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::{justify::PyJustify, line_break::PyLineBreak};

#[pycomponent(TextLayout, bridge)]
#[pyclass(name = "TextLayout", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyTextLayout {
    pub(crate) storage: ComponentStorage<TextLayout>,
}

impl PyTextLayout {
    fn default_justify() -> PyJustify {
        TextLayout::default().justify.into()
    }

    fn default_linebreak() -> PyLineBreak {
        TextLayout::default().linebreak.into()
    }
}

#[pymethods]
impl PyTextLayout {
    #[new]
    #[pyo3(signature = (
        justify = Self::default_justify(),
        linebreak = Self::default_linebreak()
    ))]
    pub fn new(justify: PyJustify, linebreak: PyLineBreak) -> (Self, PyComponent) {
        Self::from_owned(TextLayout {
            justify: justify.into(),
            linebreak: linebreak.into(),
        })
    }

    #[staticmethod]
    pub fn new_with_justify(py: Python<'_>, justify: PyJustify) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(TextLayout {
                justify: justify.into(),
                linebreak: LineBreak::WordBoundary,
            }),
        )
    }

    #[staticmethod]
    pub fn new_with_linebreak(py: Python<'_>, linebreak: PyLineBreak) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(TextLayout {
                justify: Justify::Left,
                linebreak: linebreak.into(),
            }),
        )
    }

    #[staticmethod]
    pub fn new_with_no_wrap(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(TextLayout {
                justify: Justify::Left,
                linebreak: LineBreak::NoWrap,
            }),
        )
    }

    #[getter]
    pub fn justify(&self) -> PyResult<PyJustify> {
        Ok(self.as_ref()?.justify.into())
    }

    #[setter]
    pub fn set_justify(&mut self, justify: PyJustify) -> PyResult<()> {
        self.as_mut()?.justify = justify.into();
        Ok(())
    }

    #[getter]
    pub fn linebreak(&self) -> PyResult<PyLineBreak> {
        Ok(self.as_ref()?.linebreak.into())
    }

    #[setter]
    pub fn set_linebreak(&mut self, linebreak: PyLineBreak) -> PyResult<()> {
        self.as_mut()?.linebreak = linebreak.into();
        Ok(())
    }

    #[pyo3(name = "with_justify")]
    pub fn with_justify(slf: Py<Self>, py: Python<'_>, justify: PyJustify) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.justify = justify.into();
        Ok(slf)
    }

    #[pyo3(name = "with_linebreak")]
    pub fn with_linebreak(
        slf: Py<Self>,
        py: Python<'_>,
        linebreak: PyLineBreak,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.linebreak = linebreak.into();
        Ok(slf)
    }

    #[pyo3(name = "with_no_wrap")]
    pub fn with_no_wrap(slf: Py<Self>, py: Python<'_>) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.linebreak = LineBreak::NoWrap;
        Ok(slf)
    }
}
