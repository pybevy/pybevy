use bevy::ui::Overflow;
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::prelude::*;

use crate::PyOverflowAxis;

#[pyclass(name = "Overflow", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyOverflow {
    storage: ValueStorage<Overflow>,
}

impl FromBorrowedStorage<ValueStorage<Overflow>> for PyOverflow {
    fn from_borrowed(storage: ValueStorage<Overflow>) -> Self {
        PyOverflow { storage }
    }
}

impl From<Overflow> for PyOverflow {
    fn from(overflow: Overflow) -> Self {
        PyOverflow {
            storage: ValueStorage::owned(overflow),
        }
    }
}

impl TryFrom<PyOverflow> for Overflow {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_overflow: PyOverflow) -> PyResult<Self> {
        Ok(py_overflow.storage.get()?)
    }
}

impl PyOverflow {
    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Overflow>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Overflow>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyOverflow {
    #[staticmethod]
    #[pyo3(name = "DEFAULT")]
    pub fn default_() -> Self {
        Overflow::DEFAULT.into()
    }

    #[new]
    #[pyo3(signature = (x = PyOverflowAxis::Visible, y = PyOverflowAxis::Visible))]
    pub fn py_new(x: PyOverflowAxis, y: PyOverflowAxis) -> Self {
        Overflow {
            x: x.into(),
            y: y.into(),
        }
        .into()
    }

    #[staticmethod]
    pub fn visible() -> Self {
        Overflow::visible().into()
    }

    #[staticmethod]
    pub fn clip() -> Self {
        Overflow::clip().into()
    }

    #[staticmethod]
    pub fn clip_x() -> Self {
        Overflow::clip_x().into()
    }

    #[staticmethod]
    pub fn clip_y() -> Self {
        Overflow::clip_y().into()
    }

    #[staticmethod]
    pub fn hidden() -> Self {
        Overflow::hidden().into()
    }

    #[staticmethod]
    pub fn hidden_x() -> Self {
        Overflow::hidden_x().into()
    }

    #[staticmethod]
    pub fn hidden_y() -> Self {
        Overflow::hidden_y().into()
    }

    #[staticmethod]
    pub fn scroll() -> Self {
        Overflow::scroll().into()
    }

    #[staticmethod]
    pub fn scroll_x() -> Self {
        Overflow::scroll_x().into()
    }

    #[staticmethod]
    pub fn scroll_y() -> Self {
        Overflow::scroll_y().into()
    }

    pub fn is_visible(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_visible())
    }

    #[getter]
    pub fn x(&self) -> PyResult<PyOverflowAxis> {
        Ok(self.as_ref()?.x.into())
    }

    #[setter]
    pub fn set_x(&mut self, value: PyOverflowAxis) -> PyResult<()> {
        self.as_mut()?.x = value.into();
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<PyOverflowAxis> {
        Ok(self.as_ref()?.y.into())
    }

    #[setter]
    pub fn set_y(&mut self, value: PyOverflowAxis) -> PyResult<()> {
        self.as_mut()?.y = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let o = self.as_ref()?;
        Ok(format!("Overflow(x={:?}, y={:?})", o.x, o.y))
    }
}
