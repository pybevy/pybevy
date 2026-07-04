use bevy::render::camera::TemporalJitter;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(TemporalJitter, bridge)]
#[pyclass(name = "TemporalJitter", extends = PyComponent)]
pub struct PyTemporalJitter {
    pub(crate) storage: ComponentStorage<TemporalJitter>,
}

#[pymethods]
impl PyTemporalJitter {
    #[new]
    #[pyo3(signature = (offset = PyVec2::ZERO))]
    pub fn new(offset: PyVec2) -> PyClassInitializer<Self> {
        Self::from_owned(TemporalJitter {
            offset: offset.into(),
        }).into()
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|t| &t.offset)?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.offset = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let tj = self.as_ref()?;
        Ok(format!(
            "TemporalJitter(offset=Vec2({}, {}))",
            tj.offset.x, tj.offset.y
        ))
    }
}
