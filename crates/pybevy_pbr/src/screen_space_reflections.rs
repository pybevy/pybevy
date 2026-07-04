use bevy::pbr::ScreenSpaceReflections;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(ScreenSpaceReflections, bridge, view_fields = [
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
        min_perceptual_roughness = (0.08_f32, 0.12_f32),
        max_perceptual_roughness = (0.55_f32, 0.6_f32),
        thickness = 0.25,
        linear_steps = 10,
        linear_march_exponent = 1.0,
        edge_fadeout = (0.0_f32, 0.0_f32),
        bisection_steps = 5,
        use_secant = true
    ))]
    pub fn new(
        min_perceptual_roughness: (f32, f32),
        max_perceptual_roughness: (f32, f32),
        thickness: f32,
        linear_steps: u32,
        linear_march_exponent: f32,
        edge_fadeout: (f32, f32),
        bisection_steps: u32,
        use_secant: bool,
    ) -> PyClassInitializer<Self> {
        Self::from_owned(ScreenSpaceReflections {
            min_perceptual_roughness: min_perceptual_roughness.0..min_perceptual_roughness.1,
            max_perceptual_roughness: max_perceptual_roughness.0..max_perceptual_roughness.1,
            thickness,
            linear_steps,
            linear_march_exponent,
            edge_fadeout: edge_fadeout.0..edge_fadeout.1,
            bisection_steps,
            use_secant,
        }).into()
    }

    #[getter]
    pub fn min_perceptual_roughness(&self) -> PyResult<(f32, f32)> {
        let r = &self.as_ref()?.min_perceptual_roughness;
        Ok((r.start, r.end))
    }

    #[setter]
    pub fn set_min_perceptual_roughness(&mut self, value: (f32, f32)) -> PyResult<()> {
        self.as_mut()?.min_perceptual_roughness = value.0..value.1;
        Ok(())
    }

    #[getter]
    pub fn max_perceptual_roughness(&self) -> PyResult<(f32, f32)> {
        let r = &self.as_ref()?.max_perceptual_roughness;
        Ok((r.start, r.end))
    }

    #[setter]
    pub fn set_max_perceptual_roughness(&mut self, value: (f32, f32)) -> PyResult<()> {
        self.as_mut()?.max_perceptual_roughness = value.0..value.1;
        Ok(())
    }

    #[getter]
    pub fn edge_fadeout(&self) -> PyResult<(f32, f32)> {
        let r = &self.as_ref()?.edge_fadeout;
        Ok((r.start, r.end))
    }

    #[setter]
    pub fn set_edge_fadeout(&mut self, value: (f32, f32)) -> PyResult<()> {
        self.as_mut()?.edge_fadeout = value.0..value.1;
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
