use bevy::color::{Alpha, Gray, Hue, Lcha, LinearRgba, Luminance, Mix, Srgba};
use pybevy_core::ValueStorage;
use pyo3::prelude::*;

use super::{linear_rgba::PyLinearRgba, srgba::PySrgba};

#[pyclass(name = "Lcha", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyLcha {
    storage: ValueStorage<Lcha>,
}

impl From<PyLcha> for Lcha {
    #[inline(always)]
    fn from(py_color: PyLcha) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<&PyLcha> for Lcha {
    #[inline(always)]
    fn from(py_color: &PyLcha) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<Lcha> for PyLcha {
    #[inline(always)]
    fn from(color: Lcha) -> Self {
        PyLcha::from_lcha(color)
    }
}

impl PyLcha {
    #[inline(always)]
    pub fn from_lcha(color: Lcha) -> Self {
        PyLcha {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn lcha(color: Lcha) -> Self {
        PyLcha {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Lcha> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Lcha> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyLcha {
    #[new]
    #[pyo3(signature = (lightness = 1.0, chroma = 0.0, hue = 0.0, alpha = 1.0))]
    pub fn new(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        PyLcha::lcha(Lcha::new(lightness, chroma, hue, alpha))
    }

    #[staticmethod]
    pub fn lch(lightness: f32, chroma: f32, hue: f32) -> Self {
        PyLcha::lcha(Lcha::lch(lightness, chroma, hue))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyLcha::lcha(Lcha::gray(lightness))
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
    pub fn chroma(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.chroma)
    }

    #[setter]
    pub fn set_chroma(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.chroma = value;
        Ok(())
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
    pub fn alpha(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.alpha)
    }

    #[setter]
    pub fn set_alpha(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.alpha = value;
        Ok(())
    }

    pub fn with_hue(&self, hue: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn with_chroma(&self, chroma: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_chroma(chroma)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.lighter(amount)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyLcha, factor: f32) -> PyResult<Self> {
        Ok(PyLcha::lcha(self.as_ref()?.mix(other.as_ref()?, factor)))
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
        Ok([c.lightness, c.chroma, c.hue, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.lightness, c.chroma, c.hue])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyLcha::lcha(Lcha::new(color[0], color[1], color[2], color[3]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::vec4::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.lightness, c.chroma, c.hue, c.alpha);
        Ok(pybevy_math::vec4::PyVec4::from(vec4))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::vec4::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PyLcha::lcha(Lcha::new(v.x, v.y, v.z, v.w))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::vec3::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.lightness, c.chroma, c.hue);
        Ok(pybevy_math::vec3::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::vec3::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PyLcha::lcha(Lcha::new(v.x, v.y, v.z, 1.0))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyLcha::lcha(Lcha::new(color[0], color[1], color[2], 1.0))
    }

    #[staticmethod]
    pub fn sequential_dispersed(index: u32) -> Self {
        PyLcha::lcha(Lcha::sequential_dispersed(index))
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
}
