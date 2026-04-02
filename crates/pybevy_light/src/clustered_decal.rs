use bevy::light::ClusteredDecal;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::component_storage;
use pyo3::prelude::*;
fn convert_optional_handle(
    handle: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<bevy::asset::Handle<bevy::image::Image>>> {
    match handle {
        Some(h) => {
            let py_handle = extract_handle_from_any(h)?;
            Ok(Some(py_handle.try_into()?))
        }
        None => Ok(None),
    }
}

#[component_storage(ClusteredDecal, bridge, view_fields = [tag])]
#[pyclass(name = "ClusteredDecal", extends = PyComponent)]
#[derive(Clone)]
pub struct PyClusteredDecal {
    pub(crate) storage: ComponentStorage<ClusteredDecal>,
}

#[pymethods]
impl PyClusteredDecal {
    #[new]
    #[pyo3(signature = (
        base_color_texture = None,
        normal_map_texture = None,
        metallic_roughness_texture = None,
        emissive_texture = None,
        tag = 0,
    ))]
    pub fn new(
        base_color_texture: Option<&Bound<'_, PyAny>>,
        normal_map_texture: Option<&Bound<'_, PyAny>>,
        metallic_roughness_texture: Option<&Bound<'_, PyAny>>,
        emissive_texture: Option<&Bound<'_, PyAny>>,
        tag: u32,
    ) -> PyResult<(Self, PyComponent)> {
        Ok(Self::from_owned(ClusteredDecal {
            base_color_texture: convert_optional_handle(base_color_texture)?,
            normal_map_texture: convert_optional_handle(normal_map_texture)?,
            metallic_roughness_texture: convert_optional_handle(metallic_roughness_texture)?,
            emissive_texture: convert_optional_handle(emissive_texture)?,
            tag,
        }))
    }

    #[getter]
    pub fn base_color_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .base_color_texture
            .as_ref()
            .map(PyHandle::from))
    }

    #[setter]
    pub fn set_base_color_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.base_color_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn normal_map_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .normal_map_texture
            .as_ref()
            .map(PyHandle::from))
    }

    #[setter]
    pub fn set_normal_map_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.normal_map_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn metallic_roughness_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .metallic_roughness_texture
            .as_ref()
            .map(PyHandle::from))
    }

    #[setter]
    pub fn set_metallic_roughness_texture(
        &mut self,
        texture: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.as_mut()?.metallic_roughness_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn emissive_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.emissive_texture.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_emissive_texture(&mut self, texture: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.emissive_texture = convert_optional_handle(texture)?;
        Ok(())
    }

    #[getter]
    pub fn tag(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.tag)
    }

    #[setter]
    pub fn set_tag(&mut self, tag: u32) -> PyResult<()> {
        self.as_mut()?.tag = tag;
        Ok(())
    }
}
