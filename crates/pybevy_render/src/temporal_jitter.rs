use bevy::render::camera::TemporalJitter;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pybevy_math::PyVec2;
use pyo3::prelude::*;

#[component_storage(TemporalJitter)]
#[pyclass(name = "TemporalJitter", extends = PyComponent)]
#[derive(Clone)]
pub struct PyTemporalJitter {
    pub(crate) storage: ComponentStorage<TemporalJitter>,
}

#[pymethods]
impl PyTemporalJitter {
    #[new]
    #[pyo3(signature = (offset = PyVec2::ZERO))]
    pub fn new(offset: PyVec2) -> (Self, PyComponent) {
        Self::from_owned(TemporalJitter {
            offset: offset.into(),
        })
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
