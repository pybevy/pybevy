use bevy::{
    color::{Alpha, Color, ColorToComponents, Gray, Hue, LinearRgba, Mix, Okhwba, Srgba},
    math::{Vec3, Vec4},
};
use pybevy_core::{FromBorrowedStorage, ValueStorage, repr::float_repr};
use pybevy_macros::pyvalue;
use pybevy_math::{vec3::PyVec3, vec4::PyVec4};
use pyo3::prelude::*;

use super::{color::PyColor, linear_rgba::PyLinearRgba, srgba::PySrgba};

#[pyvalue]
#[pyclass(name = "Okhwba", module = "pybevy.color", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyOkhwba {
    pub(crate) storage: ValueStorage<Okhwba>,
}

impl TryFrom<PyOkhwba> for Okhwba {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_color: PyOkhwba) -> PyResult<Self> {
        py_color.to_bevy()
    }
}

impl TryFrom<&PyOkhwba> for Okhwba {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_color: &PyOkhwba) -> PyResult<Self> {
        py_color.to_bevy()
    }
}

impl From<Okhwba> for PyOkhwba {
    #[inline(always)]
    fn from(color: Okhwba) -> Self {
        Self::from_owned(color)
    }
}

#[pymethods]
impl PyOkhwba {
    #[staticmethod]
    pub fn from_color(color: &PyColor) -> PyResult<Self> {
        Ok(Self::from_owned(Okhwba::from(Color::try_from(color)?)))
    }

    #[new]
    #[pyo3(signature = (hue = 0.0, whiteness = 0.0, blackness = 1.0, alpha = 1.0))]
    pub fn new(hue: f32, whiteness: f32, blackness: f32, alpha: f32) -> Self {
        Self::from_owned(Okhwba::new(hue, whiteness, blackness, alpha))
    }

    #[staticmethod]
    pub fn hwb(hue: f32, whiteness: f32, blackness: f32) -> Self {
        Self::from_owned(Okhwba::hwb(hue, whiteness, blackness))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        Self::from_owned(Okhwba::BLACK)
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        Self::from_owned(Okhwba::WHITE)
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        Self::from_owned(Okhwba::gray(lightness))
    }

    #[getter]
    pub fn hue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.hue)
    }

    #[setter]
    pub fn set_hue(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.hue = value;
        Ok(())
    }

    #[getter]
    pub fn whiteness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.whiteness)
    }

    #[setter]
    pub fn set_whiteness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.whiteness = value;
        Ok(())
    }

    #[getter]
    pub fn blackness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.blackness)
    }

    #[setter]
    pub fn set_blackness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.blackness = value;
        Ok(())
    }

    #[getter]
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    pub fn with_hue(&self, hue: f32) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_whiteness(&self, whiteness: f32) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_whiteness(whiteness)))
    }

    pub fn with_blackness(&self, blackness: f32) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_blackness(blackness)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &Self, factor: f32) -> PyResult<Self> {
        Ok(Self::from_owned(
            self.as_ref()?.mix(other.as_ref()?.reborrow(), factor),
        ))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?.reborrow(), factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn to_linear_rgba(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::from_owned(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_owned(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        Ok(self.as_ref()?.to_f32_array())
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        Ok(self.as_ref()?.to_f32_array_no_alpha())
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        Self::from_owned(Okhwba::from_f32_array(color))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        Self::from_owned(Okhwba::from_f32_array_no_alpha(color))
    }

    pub fn to_vec4(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from(self.as_ref()?.to_vec4()))
    }

    pub fn to_vec3(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from(self.as_ref()?.to_vec3()))
    }

    #[staticmethod]
    pub fn from_vec4(color: &PyVec4) -> PyResult<Self> {
        let value: Vec4 = color.try_into()?;
        Ok(Self::from_owned(Okhwba::from_vec4(value)))
    }

    #[staticmethod]
    pub fn from_vec3(color: &PyVec3) -> PyResult<Self> {
        let value: Vec3 = color.try_into()?;
        Ok(Self::from_owned(Okhwba::from_vec3(value)))
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    #[pyo3(name = "set_hue")]
    pub fn method_set_hue(&mut self, hue: f32) -> PyResult<()> {
        self.as_mut()?.set_hue(hue);
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let color = self.as_ref()?;
        Ok(format!(
            "Okhwba({}, {}, {}, {})",
            float_repr(color.hue),
            float_repr(color.whiteness),
            float_repr(color.blackness),
            float_repr(color.alpha),
        ))
    }
}
