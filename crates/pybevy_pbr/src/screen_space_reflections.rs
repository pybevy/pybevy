use bevy::pbr::ScreenSpaceReflections;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::range::PyRange;
use pyo3::prelude::*;

#[pycomponent(ScreenSpaceReflections, bridge, view_fields = [
    thickness,
    linear_steps,
    linear_march_exponent,
    bisection_steps,
    use_secant
])]
#[pyclass(name = "ScreenSpaceReflections", module = "pybevy.pbr", extends = PyComponent)]
pub struct PyScreenSpaceReflections {
    pub(crate) storage: ComponentStorage<ScreenSpaceReflections>,
}

#[pymethods]
impl PyScreenSpaceReflections {
    #[new]
    #[pyo3(signature = (
        *,
        min_perceptual_roughness = PyRange::from_range(0.08, 0.12),
        max_perceptual_roughness = PyRange::from_range(0.55, 0.6),
        thickness = 0.25,
        linear_steps = 10,
        linear_march_exponent = 1.0,
        edge_fadeout = PyRange::from_range(0.0, 0.0),
        bisection_steps = 5,
        use_secant = true
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        min_perceptual_roughness: PyRange,
        max_perceptual_roughness: PyRange,
        thickness: f32,
        linear_steps: u32,
        linear_march_exponent: f32,
        edge_fadeout: PyRange,
        bisection_steps: u32,
        use_secant: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(ScreenSpaceReflections {
            min_perceptual_roughness: min_perceptual_roughness.try_into()?,
            max_perceptual_roughness: max_perceptual_roughness.try_into()?,
            thickness,
            linear_steps,
            linear_march_exponent,
            edge_fadeout: edge_fadeout.try_into()?,
            bisection_steps,
            use_secant,
        })
        .into())
    }

    #[getter]
    pub fn min_perceptual_roughness(&self) -> PyResult<PyRange> {
        Ok(self
            .storage
            .borrow_field_as(|s| &s.min_perceptual_roughness)?)
    }

    #[setter]
    pub fn set_min_perceptual_roughness(&mut self, value: &PyRange) -> PyResult<()> {
        let range = value.try_into()?;
        self.as_mut()?.min_perceptual_roughness = range;
        Ok(())
    }

    #[getter]
    pub fn max_perceptual_roughness(&self) -> PyResult<PyRange> {
        Ok(self
            .storage
            .borrow_field_as(|s| &s.max_perceptual_roughness)?)
    }

    #[setter]
    pub fn set_max_perceptual_roughness(&mut self, value: &PyRange) -> PyResult<()> {
        let range = value.try_into()?;
        self.as_mut()?.max_perceptual_roughness = range;
        Ok(())
    }

    #[getter]
    pub fn edge_fadeout(&self) -> PyResult<PyRange> {
        Ok(self.storage.borrow_field_as(|s| &s.edge_fadeout)?)
    }

    #[setter]
    pub fn set_edge_fadeout(&mut self, value: &PyRange) -> PyResult<()> {
        let range = value.try_into()?;
        self.as_mut()?.edge_fadeout = range;
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
