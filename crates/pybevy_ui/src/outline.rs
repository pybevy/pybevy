use bevy::{color::Color, ui::Outline};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pycomponent(Outline, bridge)]
#[pyclass(name = "Outline", extends = PyComponent)]
#[derive(Debug)]
pub struct PyOutline {
    pub(crate) storage: ComponentStorage<Outline>,
}

#[pymethods]
impl PyOutline {
    #[new]
    #[pyo3(signature = (width = PyVal::px(1.0), offset = PyVal::zero(), color = None))]
    pub fn new(width: PyVal, offset: PyVal, color: Option<PyColor>) -> (Self, PyComponent) {
        Self::from_owned(Outline {
            width: width.into(),
            offset: offset.into(),
            color: color.map(|c| c.into()).unwrap_or(Color::WHITE),
        })
    }

    #[getter]
    pub fn width(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.width.into())
    }

    #[setter]
    pub fn set_width(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.width = value.into();
        Ok(())
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.offset.into())
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.offset = value.into();
        Ok(())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, value: PyColor) -> PyResult<()> {
        self.as_mut()?.color = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let outline = self.as_ref()?;
        Ok(format!(
            "Outline(width={:?}, offset={:?}, color={:?})",
            outline.width, outline.offset, outline.color
        ))
    }
}
