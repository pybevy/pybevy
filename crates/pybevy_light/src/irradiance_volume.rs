use bevy::{asset::Handle, image::Image, light::IrradianceVolume};
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(IrradianceVolume)]
#[pyclass(name = "IrradianceVolume", extends = PyComponent)]
#[derive(Clone)]
pub struct PyIrradianceVolume {
    pub(crate) storage: ComponentStorage<IrradianceVolume>,
}

#[pymethods]
impl PyIrradianceVolume {
    #[new]
    #[pyo3(signature = (voxels = None, intensity = 0.0, affects_lightmapped_meshes = true))]
    pub fn new(
        voxels: Option<&Bound<'_, PyAny>>,
        intensity: f32,
        affects_lightmapped_meshes: bool,
    ) -> PyResult<(Self, PyComponent)> {
        let handle = match voxels {
            Some(h) => Handle::<Image>::try_from(extract_handle_from_any(h)?)?,
            None => Handle::default(),
        };
        Ok(Self::from_owned(IrradianceVolume {
            voxels: handle,
            intensity,
            affects_lightmapped_meshes,
        }))
    }

    #[getter]
    pub fn voxels(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(self.as_ref()?.voxels.clone()))
    }

    #[setter]
    pub fn set_voxels(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.voxels = Handle::<Image>::try_from(extract_handle_from_any(value)?)?;
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

    #[getter]
    pub fn affects_lightmapped_meshes(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.affects_lightmapped_meshes)
    }

    #[setter]
    pub fn set_affects_lightmapped_meshes(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.affects_lightmapped_meshes = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let iv = self.as_ref()?;
        Ok(format!(
            "IrradianceVolume(intensity={}, affects_lightmapped_meshes={})",
            iv.intensity, iv.affects_lightmapped_meshes
        ))
    }
}
