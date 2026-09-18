use bevy::audio::Volume;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(Volume)]
#[pyclass(name = "Volume", module = "pybevy.audio", eq, frozen, from_py_object)]
#[derive(Debug, Clone, Copy)]
pub enum PyVolume {
    #[py_bevy(tuple)]
    Linear { value: f32 },
    #[py_bevy(tuple)]
    Decibels { value: f32 },
}

impl PartialEq for PyVolume {
    fn eq(&self, other: &Self) -> bool {
        self.inner() == other.inner()
    }
}

impl PyVolume {
    #[inline]
    fn inner(&self) -> Volume {
        (*self).into()
    }

    #[inline]
    fn from_volume(volume: Volume) -> Self {
        volume.into()
    }
}

#[pymethods]
impl PyVolume {
    #[staticmethod]
    #[pyo3(name = "SILENT")]
    pub fn silent() -> Self {
        Self::from_volume(Volume::SILENT)
    }

    pub fn to_linear(&self) -> f32 {
        self.inner().to_linear()
    }

    pub fn to_decibels(&self) -> f32 {
        self.inner().to_decibels()
    }

    pub fn increase_by_percentage(&self, percentage: f32) -> Self {
        Self::from_volume(self.inner().increase_by_percentage(percentage))
    }

    pub fn decrease_by_percentage(&self, percentage: f32) -> Self {
        Self::from_volume(self.inner().decrease_by_percentage(percentage))
    }

    pub fn fade_towards(&self, target: &Self, factor: f32) -> Self {
        Self::from_volume(self.inner().fade_towards(target.inner(), factor))
    }

    pub fn scale_to_factor(&self, factor: f32) -> Self {
        Self::from_volume(self.inner().scale_to_factor(factor))
    }

    fn __mul__(&self, other: &Self) -> Self {
        Self::from_volume(self.inner() * other.inner())
    }

    fn __truediv__(&self, other: &Self) -> Self {
        Self::from_volume(self.inner() / other.inner())
    }
}
