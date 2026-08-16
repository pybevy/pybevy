use bevy::color::{Alpha, Gray, Hsla, Hue, LinearRgba, Luminance, Mix, Saturation, Srgba};
use pybevy_core::{StorageMut, StorageRef, ValueStorage};
use pyo3::prelude::*;

use super::{common::fmt_f32, linear_rgba::PyLinearRgba, srgba::PySrgba};

#[pyclass(name = "Hsla", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyHsla {
    pub(crate) storage: ValueStorage<Hsla>,
}

impl TryFrom<PyHsla> for Hsla {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_color: PyHsla) -> PyResult<Self> {
        Ok(py_color.storage.get()?)
    }
}

impl TryFrom<&PyHsla> for Hsla {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_color: &PyHsla) -> PyResult<Self> {
        Ok(py_color.storage.get()?)
    }
}

impl From<Hsla> for PyHsla {
    #[inline(always)]
    fn from(color: Hsla) -> Self {
        PyHsla::from_hsla(color)
    }
}

impl PyHsla {
    #[inline(always)]
    pub fn from_hsla(color: Hsla) -> Self {
        PyHsla {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn hsla(color: Hsla) -> Self {
        PyHsla {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Hsla>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Hsla>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyHsla {
    #[new]
    #[pyo3(signature = (hue = 0.0, saturation = 0.0, lightness = 1.0, alpha = 1.0))]
    pub fn new(hue: f32, saturation: f32, lightness: f32, alpha: f32) -> Self {
        PyHsla::hsla(Hsla::new(hue, saturation, lightness, alpha))
    }

    #[staticmethod]
    pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Self {
        PyHsla::hsla(Hsla::hsl(hue, saturation, lightness))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PyHsla::hsla(Hsla::gray(lightness))
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
    pub fn lightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lightness)
    }

    #[setter]
    pub fn set_lightness(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lightness = value;
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
        Ok(PyHsla::hsla(self.as_ref()?.with_hue(hue)))
    }

    pub fn rotate_hue(&self, degrees: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.rotate_hue(degrees)))
    }

    pub fn with_saturation(&self, saturation: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_saturation(saturation)))
    }

    pub fn with_lightness(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_lightness(lightness)))
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, lightness: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_luminance(lightness)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.lighter(amount)))
    }

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn mix(&self, other: &Self, factor: f32) -> PyResult<Self> {
        Ok(PyHsla::hsla(
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
        Ok(PyLinearRgba::from_linear_rgba(linear))
    }

    pub fn to_srgba(&self) -> PyResult<PySrgba> {
        let srgba: Srgba = (*self.as_ref()?).into();
        Ok(PySrgba::from_srgba(srgba))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let c = self.as_ref()?;
        Ok([c.hue, c.saturation, c.lightness, c.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let c = self.as_ref()?;
        Ok([c.hue, c.saturation, c.lightness])
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PyHsla::hsla(Hsla::new(color[0], color[1], color[2], color[3]))
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::vec4::PyVec4> {
        use bevy::math::Vec4;
        let c = self.as_ref()?;
        let vec4 = Vec4::new(c.hue, c.saturation, c.lightness, c.alpha);
        Ok(pybevy_math::vec4::PyVec4::from(vec4))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::vec4::PyVec4) -> PyResult<Self> {
        let v: bevy::math::Vec4 = color.try_into()?;
        Ok(PyHsla::hsla(Hsla::new(v.x, v.y, v.z, v.w)))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::vec3::PyVec3> {
        use bevy::math::Vec3;
        let c = self.as_ref()?;
        let vec3 = Vec3::new(c.hue, c.saturation, c.lightness);
        Ok(pybevy_math::vec3::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::vec3::PyVec3) -> PyResult<Self> {
        let v: bevy::math::Vec3 = color.try_into()?;
        Ok(PyHsla::hsla(Hsla::new(v.x, v.y, v.z, 1.0)))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PyHsla::hsla(Hsla::new(color[0], color[1], color[2], 1.0))
    }

    #[staticmethod]
    pub fn sequential_dispersed(index: u32) -> Self {
        PyHsla::hsla(Hsla::sequential_dispersed(index))
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
            "Hsla({}, {}, {}, {})",
            fmt_f32(c.hue),
            fmt_f32(c.saturation),
            fmt_f32(c.lightness),
            fmt_f32(c.alpha),
        ))
    }
}
