use bevy::{math::Vec2, render::camera::TemporalJitter};
use pybevy_core::{ComponentStorage, PyComponent, public_error::value_out_of_range};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::{exceptions::PyValueError, prelude::*};

/// A subpixel offset outside [-0.5, 0.5] jitters the frustum past a whole pixel.
fn check_offset(offset: Vec2) -> PyResult<Vec2> {
    for (name, value) in [("offset.x", offset.x), ("offset.y", offset.y)] {
        if !(-0.5..=0.5).contains(&value) {
            return Err(PyValueError::new_err(value_out_of_range(
                name, -0.5, 0.5, value,
            )));
        }
    }
    Ok(offset)
}

#[pycomponent(TemporalJitter, bridge)]
#[pyclass(name = "TemporalJitter", module = "pybevy.render", extends = PyComponent)]
pub struct PyTemporalJitter {
    pub(crate) storage: ComponentStorage<TemporalJitter>,
}

#[pymethods]
impl PyTemporalJitter {
    #[new]
    #[pyo3(signature = (*, offset = PyVec2::ZERO))]
    pub fn new(offset: PyVec2) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(TemporalJitter {
            offset: check_offset(offset.try_into()?)?,
        })
        .into())
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|t| &t.offset)?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        let offset: Vec2 = value.try_into()?;
        // Authority first: an expired or read-only component must report that,
        // not a complaint about the value it was never allowed to store.
        let mut jitter = self.as_mut()?;
        jitter.offset = check_offset(offset)?;
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
