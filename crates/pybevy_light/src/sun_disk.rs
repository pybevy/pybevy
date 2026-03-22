use bevy::light::SunDisk;
use pybevy_core::{ComponentStorage, PyComponent};
use pyo3::prelude::*;

/// Sun disk component for directional lights.
#[pyclass(name = "SunDisk", extends = PyComponent)]
#[derive(Clone)]
pub struct PySunDisk {
    pub(crate) storage: ComponentStorage<SunDisk>,
}

impl PySunDisk {
    pub fn from_owned(value: SunDisk) -> (Self, PyComponent) {
        (
            PySunDisk {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }
    pub fn from_borrowed(storage: ComponentStorage<SunDisk>) -> (Self, PyComponent) {
        (PySunDisk { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&SunDisk> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut SunDisk> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PySunDisk {
    #[staticmethod]
    #[pyo3(name = "EARTH")]
    pub fn earth(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(SunDisk::EARTH))
    }

    #[new]
    #[pyo3(signature = (angular_size = 0.00935_f32, intensity = 1.0))]
    pub fn new(angular_size: f32, intensity: f32) -> (Self, PyComponent) {
        Self::from_owned(SunDisk {
            angular_size,
            intensity,
        })
    }

    #[getter]
    pub fn angular_size(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.angular_size)
    }

    #[setter]
    pub fn set_angular_size(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.angular_size = value;
        Ok(())
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.intensity = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let sd = self.as_ref()?;
        Ok(format!(
            "SunDisk(angular_size={}, intensity={})",
            sd.angular_size, sd.intensity
        ))
    }
}
