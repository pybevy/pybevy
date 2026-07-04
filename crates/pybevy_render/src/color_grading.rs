use bevy::render::view::{ColorGradingGlobal, ColorGradingSection};
use pybevy_core::{FromBorrowedStorage, field_storage::FieldStorage, value_storage::ValueStorage};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

#[pyclass(name = "ColorGradingSection", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyColorGradingSection {
    storage: ValueStorage<ColorGradingSection>,
}

impl Default for PyColorGradingSection {
    fn default() -> Self {
        Self {
            storage: ValueStorage::owned(ColorGradingSection::default()),
        }
    }
}

impl PyColorGradingSection {
    pub(crate) fn from_owned(section: ColorGradingSection) -> Self {
        Self {
            storage: ValueStorage::owned(section),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&ColorGradingSection> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut ColorGradingSection> {
        Ok(self.storage.as_mut()?)
    }
}

impl FromBorrowedStorage<ValueStorage<ColorGradingSection>> for PyColorGradingSection {
    fn from_borrowed(storage: ValueStorage<ColorGradingSection>) -> Self {
        Self { storage }
    }
}

#[pymethods]
impl PyColorGradingSection {
    #[new]
    #[pyo3(signature = (saturation = 1.0, contrast = 1.0, gamma = 1.0, gain = 1.0, lift = 0.0))]
    pub fn new(saturation: f32, contrast: f32, gamma: f32, gain: f32, lift: f32) -> Self {
        Self::from_owned(ColorGradingSection {
            saturation,
            contrast,
            gamma,
            gain,
            lift,
        })
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
    pub fn contrast(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.contrast)
    }

    #[setter]
    pub fn set_contrast(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.contrast = value;
        Ok(())
    }

    #[getter]
    pub fn gamma(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.gamma)
    }

    #[setter]
    pub fn set_gamma(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.gamma = value;
        Ok(())
    }

    #[getter]
    pub fn gain(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.gain)
    }

    #[setter]
    pub fn set_gain(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.gain = value;
        Ok(())
    }

    #[getter]
    pub fn lift(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.lift)
    }

    #[setter]
    pub fn set_lift(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.lift = value;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}

impl From<ColorGradingSection> for PyColorGradingSection {
    fn from(section: ColorGradingSection) -> Self {
        Self::from_owned(section)
    }
}

impl TryFrom<PyColorGradingSection> for ColorGradingSection {
    type Error = PyErr;

    fn try_from(section: PyColorGradingSection) -> PyResult<Self> {
        Ok(section.storage.get()?)
    }
}

impl TryFrom<&PyColorGradingSection> for ColorGradingSection {
    type Error = PyErr;

    fn try_from(section: &PyColorGradingSection) -> PyResult<Self> {
        Ok(section.storage.get()?)
    }
}

#[pyfield]
#[pyclass(name = "ColorGradingGlobal", from_py_object)]
#[derive(Debug)]
pub struct PyColorGradingGlobal {
    storage: FieldStorage<ColorGradingGlobal>,
}

impl Default for PyColorGradingGlobal {
    fn default() -> Self {
        Self {
            storage: FieldStorage::owned(ColorGradingGlobal::default()),
        }
    }
}

#[pymethods]
impl PyColorGradingGlobal {
    #[new]
    #[pyo3(signature = (
        exposure = 0.0,
        temperature = 0.0,
        tint = 0.0,
        hue = 0.0,
        post_saturation = 1.0,
        midtones_range = (0.2, 0.7)
    ))]
    pub fn new(
        exposure: f32,
        temperature: f32,
        tint: f32,
        hue: f32,
        post_saturation: f32,
        midtones_range: (f32, f32),
    ) -> Self {
        Self::from_owned(ColorGradingGlobal {
            exposure,
            temperature,
            tint,
            hue,
            post_saturation,
            midtones_range: midtones_range.0..midtones_range.1,
        })
    }

    #[getter]
    pub fn exposure(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.exposure)
    }

    #[setter]
    pub fn set_exposure(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.exposure = value;
        Ok(())
    }

    #[getter]
    pub fn temperature(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.temperature)
    }

    #[setter]
    pub fn set_temperature(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.temperature = value;
        Ok(())
    }

    #[getter]
    pub fn tint(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.tint)
    }

    #[setter]
    pub fn set_tint(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.tint = value;
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
    pub fn post_saturation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.post_saturation)
    }

    #[setter]
    pub fn set_post_saturation(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.post_saturation = value;
        Ok(())
    }

    #[getter]
    pub fn midtones_range(&self) -> PyResult<(f32, f32)> {
        let range = &self.as_ref()?.midtones_range;
        Ok((range.start, range.end))
    }

    #[setter]
    pub fn set_midtones_range(&mut self, value: (f32, f32)) -> PyResult<()> {
        self.as_mut()?.midtones_range = value.0..value.1;
        Ok(())
    }
}
