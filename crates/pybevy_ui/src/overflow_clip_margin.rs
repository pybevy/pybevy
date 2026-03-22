use bevy::ui::OverflowClipMargin;
use pyo3::prelude::*;

use crate::enums::PyOverflowClipBox;

#[pyclass(name = "OverflowClipMargin", frozen, eq)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PyOverflowClipMargin {
    pub(crate) inner: OverflowClipMargin,
}

impl From<OverflowClipMargin> for PyOverflowClipMargin {
    fn from(value: OverflowClipMargin) -> Self {
        PyOverflowClipMargin { inner: value }
    }
}

impl From<PyOverflowClipMargin> for OverflowClipMargin {
    fn from(value: PyOverflowClipMargin) -> Self {
        value.inner
    }
}

#[pymethods]
impl PyOverflowClipMargin {
    #[new]
    #[pyo3(signature = (visual_box = None, margin = 0.0))]
    pub fn new(visual_box: Option<PyOverflowClipBox>, margin: f32) -> Self {
        let vb = visual_box.unwrap_or(PyOverflowClipBox::PaddingBox);
        PyOverflowClipMargin {
            inner: OverflowClipMargin {
                visual_box: vb.into(),
                margin,
            },
        }
    }

    #[staticmethod]
    pub fn content_box() -> Self {
        PyOverflowClipMargin {
            inner: OverflowClipMargin::content_box(),
        }
    }

    #[staticmethod]
    pub fn padding_box() -> Self {
        PyOverflowClipMargin {
            inner: OverflowClipMargin::padding_box(),
        }
    }

    #[staticmethod]
    pub fn border_box() -> Self {
        PyOverflowClipMargin {
            inner: OverflowClipMargin::border_box(),
        }
    }

    pub fn with_margin(&self, margin: f32) -> Self {
        PyOverflowClipMargin {
            inner: self.inner.with_margin(margin),
        }
    }

    #[getter]
    pub fn visual_box(&self) -> PyOverflowClipBox {
        self.inner.visual_box.into()
    }

    #[getter]
    pub fn margin(&self) -> f32 {
        self.inner.margin
    }

    fn __repr__(&self) -> String {
        format!(
            "OverflowClipMargin(visual_box={:?}, margin={})",
            self.inner.visual_box, self.inner.margin
        )
    }
}
