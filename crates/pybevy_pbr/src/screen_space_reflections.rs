use bevy::pbr::ScreenSpaceReflections;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(ScreenSpaceReflections, bridge, view_fields = [
    perceptual_roughness_threshold,
    thickness,
    linear_steps,
    linear_march_exponent,
    bisection_steps,
    use_secant
])]
#[pyclass(name = "ScreenSpaceReflections", extends = PyComponent)]
pub struct PyScreenSpaceReflections {
    pub(crate) storage: ComponentStorage<ScreenSpaceReflections>,
}

#[pymethods]
impl PyScreenSpaceReflections {
    #[new]
    #[pyo3(signature = (
        perceptual_roughness_threshold = 0.1,
        thickness = 0.25,
        linear_steps = 16,
        linear_march_exponent = 1.0,
        bisection_steps = 4,
        use_secant = true
    ))]
    pub fn new(
        perceptual_roughness_threshold: f32,
        thickness: f32,
        linear_steps: u32,
        linear_march_exponent: f32,
        bisection_steps: u32,
        use_secant: bool,
    ) -> (Self, PyComponent) {
        Self::from_owned(ScreenSpaceReflections {
            perceptual_roughness_threshold,
            thickness,
            linear_steps,
            linear_march_exponent,
            bisection_steps,
            use_secant,
        })
    }

    #[getter]
    pub fn perceptual_roughness_threshold(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.perceptual_roughness_threshold)
    }

    #[setter]
    pub fn set_perceptual_roughness_threshold(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.perceptual_roughness_threshold = value;
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
    pub fn linear_steps(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.linear_steps)
    }

    #[setter]
    pub fn set_linear_steps(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.linear_steps = value;
        Ok(())
    }

    #[getter]
    pub fn linear_march_exponent(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.linear_march_exponent)
    }

    #[setter]
    pub fn set_linear_march_exponent(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.linear_march_exponent = value;
        Ok(())
    }

    #[getter]
    pub fn bisection_steps(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.bisection_steps)
    }

    #[setter]
    pub fn set_bisection_steps(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.bisection_steps = value;
        Ok(())
    }

    #[getter]
    pub fn use_secant(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.use_secant)
    }

    #[setter]
    pub fn set_use_secant(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.use_secant = value;
        Ok(())
    }
}
