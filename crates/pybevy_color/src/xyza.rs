use bevy::{
    color::{Alpha, Gray, LinearRgba, Luminance, Mix, Srgba, Xyza},
    math::StableInterpolate,
};
use pybevy_core::ValueStorage;
use pyo3::prelude::*;

use super::{linear_rgba::PyLinearRgba, srgba::PySrgba};

// === Xyza ===

#[pyclass(name = "Xyza", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyXyza {
    storage: ValueStorage<Xyza>,
}

impl PyXyza {
    pub fn xyza(xyza: Xyza) -> Self {
        Self {
            storage: ValueStorage::owned(xyza),
        }
    }

    fn as_ref(&self) -> PyResult<&Xyza> {
        Ok(self.storage.as_ref()?)
    }

    fn as_mut(&mut self) -> PyResult<&mut Xyza> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyXyza {
    #[new]
    #[pyo3(signature = (x = 0.0, y = 0.0, z = 0.0, alpha = 1.0))]
    pub fn new(x: f32, y: f32, z: f32, alpha: f32) -> Self {
        PyXyza::xyza(Xyza::new(x, y, z, alpha))
    }

    #[staticmethod]
    pub fn xyz(x: f32, y: f32, z: f32) -> Self {
        PyXyza::xyza(Xyza::xyz(x, y, z))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyXyza::xyza(Xyza::gray(lightness))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        PyXyza::xyza(Xyza::BLACK)
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        PyXyza::xyza(Xyza::WHITE)
    }

    #[getter]
    pub fn x(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.x = value;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.y = value;
        Ok(())
    }

    #[getter]
    pub fn z(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.z)
    }

    #[setter]
    pub fn set_z(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.z = value;
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

    pub fn with_x(&self, x: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.x = x;
        Ok(PyXyza::xyza(copy))
    }

    pub fn with_y(&self, y: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.y = y;
        Ok(PyXyza::xyza(copy))
    }

    pub fn with_z(&self, z: f32) -> PyResult<Self> {
        let mut copy = *self.as_ref()?;
        copy.z = z;
        Ok(PyXyza::xyza(copy))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.lighter(amount)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyXyza, factor: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(self.as_ref()?.mix(other.as_ref()?, factor)))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
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
        Ok([c.x, c.y, c.z, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.x, c.y, c.z])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyXyza::xyza(Xyza::new(color[0], color[1], color[2], color[3]))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyXyza::xyza(Xyza::xyz(color[0], color[1], color[2]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.x, c.y, c.z, c.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.x, c.y, c.z);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyXyza::xyza(Xyza::new(v.x, v.y, v.z, v.w))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyXyza::xyza(Xyza::xyz(v.x, v.y, v.z))
    }

    pub fn interpolate_stable(&self, other: &PyXyza, t: f32) -> PyResult<Self> {
        Ok(PyXyza::xyza(
            self.as_ref()?.interpolate_stable(other.as_ref()?, t),
        ))
    }
}
