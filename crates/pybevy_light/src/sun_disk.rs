use bevy::light::SunDisk;
use pybevy_core::{ComponentStorage, PyComponent, repr::float_repr};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(SunDisk, bridge, no_reflect, view_fields = [angular_size, intensity], domains = [angular_size => [finite], intensity => [finite]])]
#[pyclass(name = "SunDisk", module = "pybevy.light", extends = PyComponent)]
pub struct PySunDisk {
    pub(crate) storage: ComponentStorage<SunDisk>,
}

#[pymethods]
impl PySunDisk {
    #[staticmethod]
    #[pyo3(name = "EARTH")]
    pub fn earth(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(SunDisk::EARTH))
    }

    #[staticmethod]
    #[pyo3(name = "OFF")]
    pub fn off(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(SunDisk::OFF))
    }

    #[new]
    #[pyo3(signature = (*, angular_size = SunDisk::EARTH.angular_size, intensity = SunDisk::EARTH.intensity))]
    pub fn new(angular_size: f32, intensity: f32) -> PyResult<PyClassInitializer<Self>> {
        let angular_size = Self::check_angular_size(angular_size)?;
        let intensity = Self::check_intensity(intensity)?;
        Ok(Self::from_owned(SunDisk {
            angular_size,
            intensity,
        })
        .into())
    }

    #[getter]
    pub fn angular_size(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.angular_size)
    }

    #[setter]
    pub fn set_angular_size(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_angular_size(value)?;
        self.as_mut()?.angular_size = value;
        Ok(())
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_intensity(value)?;
        self.as_mut()?.intensity = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let sd = self.as_ref()?;
        Ok(format!(
            "SunDisk(angular_size={}, intensity={})",
            float_repr(sd.angular_size),
            float_repr(sd.intensity)
        ))
    }
}
