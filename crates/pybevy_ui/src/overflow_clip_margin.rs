use bevy::ui::OverflowClipMargin;
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    repr::{float_repr, object_repr},
};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::enums::PyVisualBox;

#[pyvalue]
#[pyclass(name = "OverflowClipMargin", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyOverflowClipMargin {
    pub(crate) storage: ValueStorage<OverflowClipMargin>,
}

impl PartialEq for PyOverflowClipMargin {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl From<OverflowClipMargin> for PyOverflowClipMargin {
    fn from(value: OverflowClipMargin) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyOverflowClipMargin> for OverflowClipMargin {
    type Error = PyErr;

    fn try_from(value: PyOverflowClipMargin) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyOverflowClipMargin> for OverflowClipMargin {
    type Error = PyErr;

    fn try_from(value: &PyOverflowClipMargin) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyOverflowClipMargin {
    #[new]
    #[pyo3(signature = (*, visual_box = PyVisualBox::PaddingBox, margin = 0.0))]
    pub fn new(visual_box: PyVisualBox, margin: f32) -> Self {
        Self::from_owned(OverflowClipMargin {
            visual_box: visual_box.into(),
            margin,
        })
    }

    #[staticmethod]
    pub fn content_box() -> Self {
        Self::from_owned(OverflowClipMargin::content_box())
    }

    #[staticmethod]
    pub fn padding_box() -> Self {
        Self::from_owned(OverflowClipMargin::padding_box())
    }

    #[staticmethod]
    pub fn border_box() -> Self {
        Self::from_owned(OverflowClipMargin::border_box())
    }

    pub fn with_margin(&self, margin: f32) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_margin(margin)))
    }

    #[getter]
    pub fn visual_box(&self) -> PyResult<PyVisualBox> {
        Ok(self.as_ref()?.visual_box.into())
    }

    #[setter]
    pub fn set_visual_box(&mut self, value: PyVisualBox) -> PyResult<()> {
        self.as_mut()?.visual_box = value.into();
        Ok(())
    }

    #[getter]
    pub fn margin(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.margin)
    }

    #[setter]
    pub fn set_margin(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.margin = value;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let value = self.as_ref()?;
        Ok(format!(
            "OverflowClipMargin(visual_box={}, margin={})",
            object_repr(PyVisualBox::from(value.visual_box)),
            float_repr(value.margin)
        ))
    }
}
