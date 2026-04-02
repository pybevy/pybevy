use bevy::{
    color::{
        Alpha, Gray, LinearRgba, Luminance, Mix, Oklaba, Srgba, color_difference::EuclideanDistance,
    },
    math::StableInterpolate,
};
use pybevy_core::ValueStorage;
use pyo3::prelude::*;

use super::{linear_rgba::PyLinearRgba, srgba::PySrgba};

#[pyclass(name = "Oklaba", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyOklaba {
    storage: ValueStorage<Oklaba>,
}

impl PyOklaba {
    pub fn oklaba(oklaba: Oklaba) -> Self {
        Self {
            storage: ValueStorage::owned(oklaba),
        }
    }

    fn as_ref(&self) -> PyResult<&Oklaba> {
        Ok(self.storage.as_ref()?)
    }

    fn as_mut(&mut self) -> PyResult<&mut Oklaba> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyOklaba {
    #[new]
    #[pyo3(signature = (lightness = 1.0, a = 0.0, b = 0.0, alpha = 1.0))]
    pub fn new(lightness: f32, a: f32, b: f32, alpha: f32) -> Self {
        PyOklaba::oklaba(Oklaba::new(lightness, a, b, alpha))
    }

    #[staticmethod]
    pub fn lab(lightness: f32, a: f32, b: f32) -> Self {
        PyOklaba::oklaba(Oklaba::lab(lightness, a, b))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyOklaba::oklaba(Oklaba::gray(lightness))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        PyOklaba::oklaba(Oklaba::BLACK)
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        PyOklaba::oklaba(Oklaba::WHITE)
    }

    #[getter]
    pub fn lightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lightness)
    }

    #[setter]
    pub fn set_lightness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lightness = value;
        Ok(())
    }

    #[getter]
    pub fn a(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.a)
    }

    #[setter]
    pub fn set_a(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.a = value;
        Ok(())
    }

    #[getter]
    pub fn b(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.b)
    }

    #[setter]
    pub fn set_b(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.b = value;
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

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn with_a(&self, a: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.a = a;
        Ok(PyOklaba::oklaba(copy))
    }

    pub fn with_b(&self, b: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.b = b;
        Ok(PyOklaba::oklaba(copy))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(self.as_ref()?.lighter(amount)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyOklaba, factor: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(
            self.as_ref()?.mix(other.as_ref()?, factor),
        ))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn distance(&self, other: &PyOklaba) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(other.as_ref()?))
    }

    pub fn distance_squared(&self, other: &PyOklaba) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(other.as_ref()?))
    }

    pub fn to_linear(&self) -> PyResult<PyLinearRgba> {
        let linear: LinearRgba = (*self.as_ref()?).into();
        Ok(PyLinearRgba::linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.a, c.b, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.a, c.b])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyOklaba::oklaba(Oklaba::new(color[0], color[1], color[2], color[3]))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyOklaba::oklaba(Oklaba::lab(color[0], color[1], color[2]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::vec4::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.lightness, c.a, c.b, c.alpha);
        Ok(pybevy_math::vec4::PyVec4::from(vec4))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::vec3::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.lightness, c.a, c.b);
        Ok(pybevy_math::vec3::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::vec4::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyOklaba::oklaba(Oklaba::new(v.x, v.y, v.z, v.w))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::vec3::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyOklaba::oklaba(Oklaba::lab(v.x, v.y, v.z))
    }

    pub fn interpolate_stable(&self, other: &PyOklaba, t: f32) -> PyResult<Self> {
        Ok(PyOklaba::oklaba(
            self.as_ref()?.interpolate_stable(other.as_ref()?, t),
        ))
    }
}
