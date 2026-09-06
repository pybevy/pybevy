use bevy::sprite::Text2dShadow;
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(Text2dShadow, bridge)]
#[pyclass(name = "Text2dShadow", module = "pybevy.text", extends = PyComponent)]
#[derive(Debug)]
pub struct PyText2dShadow {
    pub(crate) storage: ComponentStorage<Text2dShadow>,
}

#[pymethods]
impl PyText2dShadow {
    #[new]
    #[pyo3(signature = (offset = None, color = None))]
    pub fn new(
        offset: Option<PyVec2>,
        color: Option<PyColor>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let default = Text2dShadow::default();
        let shadow = Text2dShadow {
            offset: offset
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(default.offset),
            color: color
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(default.color),
        };

        Ok(Self::from_owned(shadow).into())
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.offset)?)
    }

    #[setter]
    pub fn set_offset(&mut self, offset: PyVec2) -> PyResult<()> {
        self.as_mut()?.offset = offset.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |shadow| &shadow.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.color = color;
        Ok(())
    }
}
