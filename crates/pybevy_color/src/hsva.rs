use bevy::{
    color::{Alpha, Gray, Hsva, Hue, LinearRgba, Mix, Saturation, Srgba},
    math::{Vec3, Vec4},
};
use pybevy_core::{StorageMut, StorageRef, ValueStorage};
use pybevy_math::{vec3::PyVec3, vec4::PyVec4};
use pyo3::prelude::*;

use super::{common::fmt_f32, linear_rgba::PyLinearRgba, srgba::PySrgba};

#[pyclass(name = "Hsva", module = "pybevy.color", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyHsva {
    pub(crate) storage: ValueStorage<Hsva>,
}

impl TryFrom<PyHsva> for Hsva {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_color: PyHsva) -> PyResult<Self> {
        Ok(py_color.storage.get()?)
    }
}

impl TryFrom<&PyHsva> for Hsva {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_color: &PyHsva) -> PyResult<Self> {
        Ok(py_color.storage.get()?)
    }
}

impl From<Hsva> for PyHsva {
    #[inline(always)]
    fn from(color: Hsva) -> Self {
        PyHsva::from_hsva(color)
    }
}

impl PyHsva {
    #[inline(always)]
    pub fn from_hsva(color: Hsva) -> Self {
        PyHsva {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn hsva(color: Hsva) -> Self {
        PyHsva {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Hsva>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Hsva>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyHsva {
    #[new]
    #[pyo3(signature = (hue = 0.0, saturation = 0.0, value = 1.0, alpha = 1.0))]
    pub fn new(hue: f32, saturation: f32, value: f32, alpha: f32) -> Self {
        PyHsva::hsva(Hsva::new(hue, saturation, value, alpha))
    }

    #[staticmethod]
    pub fn hsv(hue: f32, saturation: f32, value: f32) -> Self {
        PyHsva::hsva(Hsva::hsv(hue, saturation, value))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyHsva::hsva(Hsva::gray(lightness))
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
    pub fn saturation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.saturation)
    }

    #[setter]
    pub fn set_saturation(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.saturation = value;
        Ok(())
    }

    #[getter]
    pub fn value(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.value)
    }

    #[setter]
    pub fn set_value(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.value = value;
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
        Ok(PyHsva::hsva(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_saturation(&self, saturation: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.with_saturation(saturation)))
    }

    pub fn with_value(&self, value: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.with_value(value)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyHsva, factor: f32) -> PyResult<Self> {
        Ok(PyHsva::hsva(
            self.as_ref()?.mix(other.as_ref()?.reborrow(), factor),
        ))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?.reborrow(), factor);
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
        Ok([c.hue, c.saturation, c.value, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.hue, c.saturation, c.value])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyHsva::hsva(Hsva::new(color[0], color[1], color[2], color[3]))
    }

    pub fn to_vec4(&self) -> PyResult<PyVec4> {
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.hue, c.saturation, c.value, c.alpha);
        Ok(PyVec4::from(vec4))
    }

    #[staticmethod]
    pub fn from_vec4(color: &PyVec4) -> PyResult<Self> {
        let v: Vec4 = color.try_into()?;
        Ok(PyHsva::hsva(Hsva::new(v.x, v.y, v.z, v.w)))
    }

    pub fn to_vec3(&self) -> PyResult<PyVec3> {
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.hue, c.saturation, c.value);
        Ok(PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec3(color: &PyVec3) -> PyResult<Self> {
        let v: Vec3 = color.try_into()?;
        Ok(PyHsva::hsva(Hsva::new(v.x, v.y, v.z, 1.0)))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyHsva::hsva(Hsva::new(color[0], color[1], color[2], 1.0))
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

    #[pyo3(name = "set_saturation")]
    pub fn method_set_saturation(&mut self, saturation: f32) -> PyResult<()> {
        self.as_mut()?.set_saturation(saturation);
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let c = self.as_ref()?;
        Ok(format!(
            "Hsva({}, {}, {}, {})",
            fmt_f32(c.hue),
            fmt_f32(c.saturation),
            fmt_f32(c.value),
            fmt_f32(c.alpha),
        ))
    }
}
