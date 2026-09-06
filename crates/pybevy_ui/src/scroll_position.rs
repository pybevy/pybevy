use bevy::ui::ScrollPosition;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(ScrollPosition, bridge, view_fields = [0.x as x, 0.y as y])]
#[pyclass(name = "ScrollPosition", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyScrollPosition {
    pub(crate) storage: ComponentStorage<ScrollPosition>,
}

#[pymethods]
impl PyScrollPosition {
    #[new]
    #[pyo3(signature = (x = 0.0, y = 0.0))]
    pub fn new(x: f32, y: f32) -> PyClassInitializer<Self> {
        Self::from_owned(ScrollPosition(bevy::math::Vec2::new(x, y))).into()
    }

    #[getter]
    pub fn x(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.0.x)
    }

    #[setter]
    pub fn set_x(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.0.x = value;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.0.y)
    }

    #[setter]
    pub fn set_y(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.0.y = value;
        Ok(())
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.0)?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.0 = value.try_into()?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let pos = self.as_ref()?;
        Ok(format!("ScrollPosition({}, {})", pos.0.x, pos.0.y))
    }
}
