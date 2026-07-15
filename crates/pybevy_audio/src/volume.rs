use bevy::audio::Volume;
use pyo3::prelude::*;

#[pyclass(name = "Volume", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyVolume {
    Linear { value: f32 },
    Decibels { value: f32 },
}

impl From<Volume> for PyVolume {
    fn from(volume: Volume) -> Self {
        match volume {
            Volume::Linear(value) => PyVolume::Linear { value },
            Volume::Decibels(value) => PyVolume::Decibels { value },
        }
    }
}

impl From<PyVolume> for Volume {
    fn from(volume: PyVolume) -> Self {
        match volume {
            PyVolume::Linear { value } => Volume::Linear(value),
            PyVolume::Decibels { value } => Volume::Decibels(value),
        }
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

    fn __repr__(&self) -> String {
        match self {
            PyVolume::Linear { value } => format!("Volume.Linear({value})"),
            PyVolume::Decibels { value } => format!("Volume.Decibels({value})"),
        }
    }
}
