use bevy::pbr::ContactShadows;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(ContactShadows, bridge, view_fields = [
    linear_steps,
    thickness,
    length
])]
#[pyclass(name = "ContactShadows", extends = PyComponent)]
pub struct PyContactShadows {
    pub(crate) storage: ComponentStorage<ContactShadows>,
}

#[pymethods]
impl PyContactShadows {
    #[new]
    #[pyo3(signature = (
        linear_steps = 16,
        thickness = 0.1,
        length = 0.3
    ))]
    pub fn new(linear_steps: u32, thickness: f32, length: f32) -> PyClassInitializer<Self> {
        Self::from_owned(ContactShadows {
            linear_steps,
            thickness,
            length,
        })
        .into()
    }

    #[getter]
    pub fn linear_steps(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.linear_steps)
    }

    #[setter]
    pub fn set_linear_steps(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.linear_steps = value;
        Ok(())
    }

    #[getter]
    pub fn thickness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.thickness)
    }

    #[setter]
    pub fn set_thickness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.thickness = value;
        Ok(())
    }

    #[getter]
    pub fn length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length)
    }

    #[setter]
    pub fn set_length(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.length = value;
        Ok(())
    }
}
