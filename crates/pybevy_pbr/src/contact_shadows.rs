use bevy::pbr::ContactShadows;
use pybevy_core::{ComponentStorage, PyComponent, domain::IntegerInput};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(ContactShadows, bridge, view_fields = [
    linear_steps,
    thickness,
    length
], domains = [
    linear_steps => [range_inclusive(0.0, 1024.0)],
    length => [finite],
    thickness => [finite],
])]
#[pyclass(name = "ContactShadows", module = "pybevy.pbr", extends = PyComponent)]
pub struct PyContactShadows {
    pub(crate) storage: ComponentStorage<ContactShadows>,
}

#[pymethods]
impl PyContactShadows {
    #[new]
    #[pyo3(signature = (
        *,
        linear_steps = IntegerInput::new(16),
        thickness = 0.1,
        length = 0.3
    ))]
    pub fn new(
        linear_steps: IntegerInput<u32>,
        thickness: f32,
        length: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let linear_steps = Self::check_linear_steps(linear_steps)?.stored();
        let length = Self::check_length(length)?;
        let thickness = Self::check_thickness(thickness)?;
        Ok(Self::from_owned(ContactShadows {
            linear_steps,
            thickness,
            length,
        })
        .into())
    }

    #[getter]
    pub fn linear_steps(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.linear_steps)
    }

    #[setter]
    pub fn set_linear_steps(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        let value = Self::check_linear_steps(value)?.stored();
        self.as_mut()?.linear_steps = value;
        Ok(())
    }

    #[getter]
    pub fn thickness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.thickness)
    }

    #[setter]
    pub fn set_thickness(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_thickness(value)?;
        self.as_mut()?.thickness = value;
        Ok(())
    }

    #[getter]
    pub fn length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length)
    }

    #[setter]
    pub fn set_length(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_length(value)?;
        self.as_mut()?.length = value;
        Ok(())
    }
}
