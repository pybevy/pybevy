use bevy::light::Skybox;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::quat::PyQuat;
use pyo3::prelude::*;

#[pycomponent(Skybox, bridge, view_fields = [brightness])]
#[pyclass(name = "Skybox", extends = PyComponent)]
pub struct PySkybox {
    pub(crate) storage: ComponentStorage<Skybox>,
}

#[pymethods]
impl PySkybox {
    #[new]
    #[pyo3(signature = (
        image = None,
        brightness = Skybox::default().brightness,
        rotation = Skybox::default().rotation.into()
    ))]
    pub fn new(
        image: Option<&Bound<'_, PyAny>>,
        brightness: f32,
        rotation: PyQuat,
    ) -> PyResult<PyClassInitializer<Self>> {
        let image = match image {
            Some(image) => Some(extract_handle_from_any(image)?.try_into()?),
            None => None,
        };
        Ok(Self::from_owned(Skybox {
            image,
            brightness,
            rotation: rotation.into(),
        })
        .into())
    }

    #[getter]
    pub fn image(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.image.clone().map(Into::into))
    }

    #[setter]
    pub fn set_image(&mut self, image: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.image = match image {
            Some(image) => Some(extract_handle_from_any(image)?.try_into()?),
            None => None,
        };
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
