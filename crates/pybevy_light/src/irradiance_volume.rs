use bevy::{asset::Handle, image::Image, light::IrradianceVolume};
use pybevy_core::{
    ComponentStorage, HandleArg, PyComponent, PyHandle, extract_handle_from_any, repr::float_repr,
};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

// The default voxel handle is a 2D image; the shader binds a 3D one, so it cannot render.
#[pycomponent(
    IrradianceVolume,
    bridge,
    no_spawn_example,
    view_fields = [intensity, affects_lightmapped_meshes]
)]
#[pyclass(name = "IrradianceVolume", module = "pybevy.light", extends = PyComponent)]
pub struct PyIrradianceVolume {
    pub(crate) storage: ComponentStorage<IrradianceVolume>,
}

#[pymethods]
impl PyIrradianceVolume {
    #[new]
    #[pyo3(signature = (*, voxels = HandleArg::default(), intensity = 0.0, affects_lightmapped_meshes = true))]
    pub fn new(
        voxels: HandleArg,
        intensity: f32,
        affects_lightmapped_meshes: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(IrradianceVolume {
            voxels: voxels.into_handle()?,
            intensity,
            affects_lightmapped_meshes,
        })
        .into())
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
            float_repr(iv.intensity),
            if iv.affects_lightmapped_meshes {
                "True"
            } else {
                "False"
            }
        ))
    }
}
