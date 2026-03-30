use bevy::{
    color::{Alpha, Gray, Luminance, Mix, Srgba, color_difference::EuclideanDistance},
    math::StableInterpolate,
};
use pybevy_core::ValueStorage;
use pyo3::{exceptions::PyValueError, prelude::*};

use super::common::fmt_f32;

#[pyclass(name = "Srgba", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PySrgba {
    storage: ValueStorage<Srgba>,
}

impl From<PySrgba> for Srgba {
    #[inline(always)]
    fn from(py_color: PySrgba) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<&PySrgba> for Srgba {
    #[inline(always)]
    fn from(py_color: &PySrgba) -> Self {
        py_color.storage.get().unwrap()
    }
}

impl From<Srgba> for PySrgba {
    #[inline(always)]
    fn from(color: Srgba) -> Self {
        PySrgba::from_srgba(color)
    }
}

impl PySrgba {
    #[inline(always)]
    pub fn from_srgba(color: Srgba) -> Self {
        PySrgba {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    pub const fn srgba(color: Srgba) -> Self {
        PySrgba {
            storage: ValueStorage::owned(color),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Srgba> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Srgba> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PySrgba {
    #[new]
    #[pyo3(signature = (red = 1.0, green = 1.0, blue = 1.0, alpha = 1.0))]
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        PySrgba::srgba(Srgba::new(red, green, blue, alpha))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let c = self.as_ref()?;
        Ok(format!(
            "Srgba({}, {}, {}, {})",
            fmt_f32(c.red),
            fmt_f32(c.green),
            fmt_f32(c.blue),
            fmt_f32(c.alpha),
        ))
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> Self {
        Self::srgba(Srgba::BLACK)
    }
    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> Self {
        Self::srgba(Srgba::WHITE)
    }
    #[staticmethod]
    #[pyo3(name = "NONE")]
    pub fn none_() -> Self {
        Self::srgba(Srgba::NONE)
    }

    #[staticmethod]
    pub fn rgb(red: f32, green: f32, blue: f32) -> Self {
        PySrgba::srgba(Srgba::rgb(red, green, blue))
    }

    #[staticmethod]
    pub fn gray(lightness: f32) -> Self {
        PySrgba::srgba(Srgba::gray(lightness))
    }

    #[staticmethod]
    pub fn rgb_u8(r: u8, g: u8, b: u8) -> Self {
        PySrgba::srgba(Srgba::rgb_u8(r, g, b))
    }

    #[staticmethod]
    pub fn rgba_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        PySrgba::srgba(Srgba::rgba_u8(r, g, b, a))
    }

    #[staticmethod]
    pub fn hex(hex: &str) -> PyResult<Self> {
        Srgba::hex(hex)
            .map(PySrgba::srgba)
            .map_err(|e| PyValueError::new_err(format!("Invalid hex color: {:?}", e)))
    }

    pub fn to_hex(&self) -> PyResult<String> {
        Ok(self.as_ref()?.to_hex())
    }

    pub fn with_red(&self, red: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_red(red)))
    }

    pub fn with_green(&self, green: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_green(green)))
    }

    pub fn with_blue(&self, blue: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_blue(blue)))
    }

    #[getter]
    pub fn red(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.red)
    }

    #[setter]
    pub fn set_red(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.red = value;
        Ok(())
    }

    #[getter]
    pub fn green(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.green)
    }

    #[setter]
    pub fn set_green(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.green = value;
        Ok(())
    }

    #[getter]
    pub fn blue(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.blue)
    }

    #[setter]
    pub fn set_blue(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.blue = value;
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

    pub fn with_alpha(&self, alpha: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_alpha(alpha)))
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn is_fully_opaque(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_opaque())
    }

    pub fn luminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.luminance())
    }

    pub fn with_luminance(&self, luminance: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.with_luminance(luminance)))
    }

    pub fn darker(&self, amount: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.darker(amount)))
    }

    pub fn lighter(&self, amount: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.lighter(amount)))
    }

    pub fn mix(&self, other: &Self, factor: f32) -> PyResult<Self> {
        Ok(PySrgba::srgba(self.as_ref()?.mix(other.as_ref()?, factor)))
    }

    pub fn mix_assign(&mut self, other: &Self, factor: f32) -> PyResult<()> {
        let result = self.as_ref()?.mix(other.as_ref()?, factor);
        *self.as_mut()? = result;
        Ok(())
    }

    pub fn distance(&self, other: &Self) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(other.as_ref()?))
    }

    pub fn distance_squared(&self, other: &Self) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(other.as_ref()?))
    }

    pub fn to_f32_array(&self) -> PyResult<[f32; 4]> {
        let color = self.as_ref()?;
        Ok([color.red, color.green, color.blue, color.alpha])
    }

    pub fn to_f32_array_no_alpha(&self) -> PyResult<[f32; 3]> {
        let color = self.as_ref()?;
        Ok([color.red, color.green, color.blue])
    }

    pub fn to_vec4(&self) -> PyResult<pybevy_math::PyVec4> {
        use bevy::math::Vec4;
        let color = self.as_ref()?;
        let vec4 = Vec4::new(color.red, color.green, color.blue, color.alpha);
        Ok(pybevy_math::PyVec4::from(vec4))
    }

    pub fn to_vec3(&self) -> PyResult<pybevy_math::PyVec3> {
        use bevy::math::Vec3;
        let color = self.as_ref()?;
        let vec3 = Vec3::new(color.red, color.green, color.blue);
        Ok(pybevy_math::PyVec3::from(vec3))
    }

    #[staticmethod]
    pub fn from_f32_array(color: [f32; 4]) -> Self {
        PySrgba::from_srgba(Srgba::new(color[0], color[1], color[2], color[3]))
    }

    #[staticmethod]
    pub fn from_f32_array_no_alpha(color: [f32; 3]) -> Self {
        PySrgba::from_srgba(Srgba::rgb(color[0], color[1], color[2]))
    }

    #[staticmethod]
    pub fn from_vec4(color: &pybevy_math::PyVec4) -> Self {
        let v: bevy::math::Vec4 = color.into();
        PySrgba::from_srgba(Srgba::new(v.x, v.y, v.z, v.w))
    }

    #[staticmethod]
    pub fn from_vec3(color: &pybevy_math::PyVec3) -> Self {
        let v: bevy::math::Vec3 = color.into();
        PySrgba::from_srgba(Srgba::rgb(v.x, v.y, v.z))
    }

    pub fn to_u8_array(&self) -> PyResult<[u8; 4]> {
        let color = self.as_ref()?;
        Ok([
            (color.red * 255.0) as u8,
            (color.green * 255.0) as u8,
            (color.blue * 255.0) as u8,
            (color.alpha * 255.0) as u8,
        ])
    }

    pub fn to_u8_array_no_alpha(&self) -> PyResult<[u8; 3]> {
        let color = self.as_ref()?;
        Ok([
            (color.red * 255.0) as u8,
            (color.green * 255.0) as u8,
            (color.blue * 255.0) as u8,
        ])
    }

    #[staticmethod]
    pub fn from_u8_array(color: [u8; 4]) -> Self {
        PySrgba::from_srgba(Srgba::new(
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ))
    }

    #[staticmethod]
    pub fn from_u8_array_no_alpha(color: [u8; 3]) -> Self {
        PySrgba::from_srgba(Srgba::rgb(
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ))
    }

    pub fn interpolate_stable(&self, other: &Self, t: f32) -> PyResult<Self> {
        Ok(PySrgba::from_srgba(
            self.as_ref()?.interpolate_stable(other.as_ref()?, t),
        ))
    }

    #[pyo3(name = "set_alpha")]
    pub fn method_set_alpha(&mut self, alpha: f32) -> PyResult<()> {
        self.as_mut()?.set_alpha(alpha);
        Ok(())
    }

    #[staticmethod]
    pub fn gamma_function(value: f32) -> f32 {
        Srgba::gamma_function(value)
    }

    #[staticmethod]
    pub fn gamma_function_inverse(value: f32) -> f32 {
        Srgba::gamma_function_inverse(value)
    }
}
