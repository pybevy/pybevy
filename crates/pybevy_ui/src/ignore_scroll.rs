use bevy::ui::IgnoreScroll;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::bvec2::PyBVec2;
use pyo3::prelude::*;

#[pycomponent(IgnoreScroll, bridge, view_fields = [0.x as x, 0.y as y])]
#[pyclass(name = "IgnoreScroll", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyIgnoreScroll {
    pub(crate) storage: ComponentStorage<IgnoreScroll>,
}

#[pymethods]
impl PyIgnoreScroll {
    #[new]
    #[pyo3(signature = (value = PyBVec2::from(IgnoreScroll::default().0)))]
    pub fn new(value: PyBVec2) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(IgnoreScroll(value.try_into()?)).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyBVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|ignore| &ignore.0, |ignore| &mut ignore.0)?)
    }

    #[setter]
    pub fn set_value(&mut self, value: PyBVec2) -> PyResult<()> {
        self.as_mut()?.0 = value.try_into()?;
        Ok(())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()?.0 == other.as_ref()?.0)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "IgnoreScroll({})",
            PyBVec2::from(self.as_ref()?.0).__repr__()?
        ))
    }
}
