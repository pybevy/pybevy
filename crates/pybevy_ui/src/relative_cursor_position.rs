use bevy::ui::RelativeCursorPosition;
use pybevy_core::{ComponentStorage, FromBorrowedStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(RelativeCursorPosition, bridge)]
#[pyclass(name = "RelativeCursorPosition", module = "pybevy.ui", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyRelativeCursorPosition {
    pub(crate) storage: ComponentStorage<RelativeCursorPosition>,
}

#[pymethods]
impl PyRelativeCursorPosition {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(RelativeCursorPosition::default()).into()
    }

    #[getter]
    pub fn cursor_over(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.cursor_over)
    }

    #[setter]
    pub fn set_cursor_over(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.cursor_over = value;
        Ok(())
    }

    #[getter]
    pub fn normalized(&self) -> PyResult<Option<PyVec2>> {
        Ok(self
            .storage
            .borrow_optional_field(|r| &r.normalized)?
            .map(PyVec2::from_borrowed))
    }

    #[setter]
    pub fn set_normalized(&mut self, value: Option<PyVec2>) -> PyResult<()> {
        self.as_mut()?.normalized = value.map(TryInto::try_into).transpose()?;
        Ok(())
    }

    pub fn is_cursor_over(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.cursor_over())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let inner = self.as_ref()?;
        Ok(format!(
            "RelativeCursorPosition(cursor_over={}, normalized={:?})",
            inner.cursor_over, inner.normalized
        ))
    }
}
