use bevy::core_pipeline::Skybox;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::quat::PyQuat;
use pyo3::prelude::*;

#[pycomponent(Skybox, bridge, view_fields = [brightness])]
#[pyclass(name = "Skybox", extends = PyComponent)]
#[derive(Clone)]
pub struct PySkybox {
    pub(crate) storage: ComponentStorage<Skybox>,
}

#[pymethods]
impl PySkybox {
    #[new]
    #[pyo3(signature = (
        image,
        brightness = Skybox::default().brightness,
        rotation = Skybox::default().rotation.into()
    ))]
    pub fn new(
        image: &Bound<'_, PyAny>,
        brightness: f32,
        rotation: PyQuat,
    ) -> PyResult<(Self, PyComponent)> {
        // Extract handle from any Handle-like object (supports both pybevy_core and main crate handles)
        let handle = extract_handle_from_any(image)?;
        Ok(Self::from_owned(Skybox {
            image: handle.try_into()?,
            brightness,
            rotation: rotation.into(),
        }))
    }

    #[getter]
    pub fn image(&self) -> PyResult<PyHandle> {
        Ok(self.as_ref()?.image.clone().into())
    }

    #[setter]
    pub fn set_image(&mut self, image: &Bound<'_, PyAny>) -> PyResult<()> {
        let handle = extract_handle_from_any(image)?;
        self.as_mut()?.image = handle.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn brightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.brightness)
    }

    #[setter]
    pub fn set_brightness(&mut self, brightness: f32) -> PyResult<()> {
        self.as_mut()?.brightness = brightness;
        Ok(())
    }

    #[getter]
    pub fn rotation(&self) -> PyResult<PyQuat> {
        Ok(self.storage.borrow_field_as(|s| &s.rotation)?)
    }

    #[setter]
    pub fn set_rotation(&mut self, rotation: PyQuat) -> PyResult<()> {
        self.as_mut()?.rotation = rotation.into();
        Ok(())
    }
}
