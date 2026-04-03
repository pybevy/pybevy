use bevy::light::{DirectionalLightTexture, PointLightTexture, SpotLightTexture};
use pybevy_camera::cubemap_layout::PyCubemapLayout;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(DirectionalLightTexture, bridge, view_only_fields = [tiled: bool])]
#[pyclass(name = "DirectionalLightTexture", extends = PyComponent)]
#[derive(Clone)]
pub struct PyDirectionalLightTexture {
    pub(crate) storage: ComponentStorage<DirectionalLightTexture>,
}

#[pymethods]
impl PyDirectionalLightTexture {
    #[new]
    pub fn new(image: &Bound<'_, PyAny>, tiled: bool) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(image)?;
        Ok(Self::from_owned(DirectionalLightTexture {
            image: handle.try_into()?,
            tiled,
        }))
    }

    #[getter]
    pub fn image(&self) -> PyResult<PyHandle> {
        Ok(self.as_ref()?.image.clone().into())
    }

    #[setter]
    pub fn set_image(&mut self, image: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.image = extract_handle_from_any(image)?.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn tiled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.tiled)
    }

    #[setter]
    pub fn set_tiled(&mut self, tiled: bool) -> PyResult<()> {
        self.as_mut()?.tiled = tiled;
        Ok(())
    }
}

#[pycomponent(SpotLightTexture, bridge)]
#[pyclass(name = "SpotLightTexture", extends = PyComponent)]
#[derive(Clone)]
pub struct PySpotLightTexture {
    pub(crate) storage: ComponentStorage<SpotLightTexture>,
}

#[pymethods]
impl PySpotLightTexture {
    #[new]
    pub fn new(image: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(image)?;
        Ok(Self::from_owned(SpotLightTexture {
            image: handle.try_into()?,
        }))
    }

    #[getter]
    pub fn image(&self) -> PyResult<PyHandle> {
        Ok(self.as_ref()?.image.clone().into())
    }

    #[setter]
    pub fn set_image(&mut self, image: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.image = extract_handle_from_any(image)?.try_into()?;
        Ok(())
    }
}

#[pycomponent(PointLightTexture, bridge)]
#[pyclass(name = "PointLightTexture", extends = PyComponent)]
#[derive(Clone)]
pub struct PyPointLightTexture {
    pub(crate) storage: ComponentStorage<PointLightTexture>,
}

#[pymethods]
impl PyPointLightTexture {
    #[new]
    pub fn new(
        image: &Bound<'_, PyAny>,
        cubemap_layout: PyCubemapLayout,
    ) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(image)?;
        Ok(Self::from_owned(PointLightTexture {
            image: handle.try_into()?,
            cubemap_layout: cubemap_layout.into(),
        }))
    }

    #[getter]
    pub fn image(&self) -> PyResult<PyHandle> {
        Ok(self.as_ref()?.image.clone().into())
    }

    #[setter]
    pub fn set_image(&mut self, image: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.image = extract_handle_from_any(image)?.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn cubemap_layout(&self) -> PyResult<PyCubemapLayout> {
        Ok(self.as_ref()?.cubemap_layout.into())
    }

    #[setter]
    pub fn set_cubemap_layout(&mut self, layout: PyCubemapLayout) -> PyResult<()> {
        self.as_mut()?.cubemap_layout = layout.into();
        Ok(())
    }
}
