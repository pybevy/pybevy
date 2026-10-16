use bevy::{math::Vec2, render::camera::TemporalJitter};
use pybevy_core::{ComponentStorage, PyComponent, repr::ReprLiteral};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

// A subpixel offset outside [-0.5, 0.5] jitters the frustum past a whole pixel.
#[pycomponent(
    TemporalJitter,
    bridge,
    domains = [offset => [range_inclusive(-0.5, 0.5)]]
)]
#[pyclass(name = "TemporalJitter", module = "pybevy.render", extends = PyComponent)]
pub struct PyTemporalJitter {
    pub(crate) storage: ComponentStorage<TemporalJitter>,
}

#[pymethods]
impl PyTemporalJitter {
    #[new]
    #[pyo3(signature = (*, offset = PyVec2::ZERO))]
    pub fn new(offset: PyVec2) -> PyResult<PyClassInitializer<Self>> {
        let offset = Self::check_offset(Vec2::try_from(offset)?)?;
        Ok(Self::from_owned(TemporalJitter { offset }).into())
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |t| &t.offset,
            |t| &mut t.offset,
            Self::validate_offset,
        )?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        let offset: Vec2 = value.try_into()?;
        self.as_mut()?.offset = Self::check_offset(offset)?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let tj = self.as_ref()?;
        Ok(format!(
            "TemporalJitter(offset={})",
            tj.offset.repr_literal()
        ))
    }
}
