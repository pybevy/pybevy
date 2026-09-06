use bevy::{
    color::{
        Alpha, Gray, Hue, LinearRgba, Luminance, Mix, Oklcha, Srgba,
        color_difference::EuclideanDistance,
    },
    math::{Vec3, Vec4},
};
use pybevy_core::{StorageMut, StorageRef, ValueStorage};
use pybevy_math::{vec3::PyVec3, vec4::PyVec4};
use pyo3::prelude::*;

use super::{common::fmt_f32, linear_rgba::PyLinearRgba, srgba::PySrgba};

#[pyclass(name = "Oklcha", module = "pybevy.color", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyOklcha {
    pub(crate) storage: ValueStorage<Oklcha>,
}

impl TryFrom<PyOklcha> for Oklcha {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_color: PyOklcha) -> PyResult<Self> {
        Ok(py_color.storage.get()?)
    }
}

impl TryFrom<&PyOklcha> for Oklcha {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_color: &PyOklcha) -> PyResult<Self> {
        Ok(py_color.storage.get()?)
    }
}

impl From<Oklcha> for PyOklcha {
    #[inline(always)]
    fn from(color: Oklcha) -> Self {
        PyOklcha::from_oklcha(color)
    }
}

impl PyOklcha {
    #[inline(always)]
    pub fn from_oklcha(color: Oklcha) -> Self {
        PyOklcha {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn oklcha(color: Oklcha) -> Self {
        PyOklcha {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Oklcha>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Oklcha>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyOklcha {
    #[new]
    #[pyo3(signature = (lightness = 1.0, chroma = 0.0, hue = 0.0, alpha = 1.0))]
    pub fn new(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        PyOklcha::oklcha(Oklcha::new(lightness, chroma, hue, alpha))
    }

    #[staticmethod]
    pub fn lch(lightness: f32, chroma: f32, hue: f32) -> Self {
        PyOklcha::oklcha(Oklcha::lch(lightness, chroma, hue))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyOklcha::oklcha(Oklcha::gray(lightness))
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
        Ok(PyOklcha::oklcha(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn with_chroma(&self, chroma: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_chroma(chroma)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.lighter(amount)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &PyOklcha, factor: f32) -> PyResult<Self> {
        Ok(PyOklcha::oklcha(
            self.as_ref()?.mix(other.as_ref()?.reborrow(), factor),
        ))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?.reborrow(), factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn distance(&self, other: &PyOklcha) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(other.as_ref()?.reborrow()))
    }

    pub fn distance_squared(&self, other: &PyOklcha) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(other.as_ref()?.reborrow()))
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
        PyOklcha::oklcha(Oklcha::new(color[0], color[1], color[2], color[3]))
    }

    pub fn to_vec4(&self) -> PyResult<PyVec4> {
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.lightness, c.chroma, c.hue, c.alpha);
        Ok(PyVec4::from(vec4))
    }

    #[staticmethod]
    pub fn from_vec4(color: &PyVec4) -> PyResult<Self> {
        let v: Vec4 = color.try_into()?;
        Ok(PyOklcha::oklcha(Oklcha::new(v.x, v.y, v.z, v.w)))
    }

    pub fn to_vec3(&self) -> PyResult<PyVec3> {
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.lightness, c.chroma, c.hue);
        Ok(PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec3(color: &PyVec3) -> PyResult<Self> {
        let v: Vec3 = color.try_into()?;
        Ok(PyOklcha::oklcha(Oklcha::new(v.x, v.y, v.z, 1.0)))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyOklcha::oklcha(Oklcha::new(color[0], color[1], color[2], 1.0))
    }

    #[staticmethod]
    pub fn sequential_dispersed(index: u32) -> Self {
        PyOklcha::oklcha(Oklcha::sequential_dispersed(index))
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
        let c = self.as_ref()?;
        Ok(format!(
            "Oklcha({}, {}, {}, {})",
            fmt_f32(c.lightness),
            fmt_f32(c.chroma),
            fmt_f32(c.hue),
            fmt_f32(c.alpha),
        ))
    }
}
