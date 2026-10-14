use bevy::text::{DEFAULT_REM_SIZE_PX, EmSize};
use pybevy_core::{ComponentStorage, PyComponent, repr::float_repr};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(
    EmSize,
    bridge,
    domains = [0 as value => [positive, range_inclusive(0.0, 4096.0)]]
)]
#[pyclass(name = "EmSize", module = "pybevy.text", extends = PyComponent, skip_from_py_object)]
#[derive(Debug)]
pub struct PyEmSize {
    pub(crate) storage: ComponentStorage<EmSize>,
}

#[pymethods]
impl PyEmSize {
    #[new]
    #[pyo3(signature = (value = DEFAULT_REM_SIZE_PX))]
    pub fn new(value: f32) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(EmSize(Self::check_value(value)?)).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_value(value)?;
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("EmSize({})", float_repr(self.as_ref()?.0)))
    }
}
