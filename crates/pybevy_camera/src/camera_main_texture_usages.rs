use bevy::{camera::CameraMainTextureUsages, render::render_resource::TextureUsages};
use pybevy_core::{ComponentStorage, PyComponent, StorageError};
use pybevy_macros::pycomponent;
use pybevy_render::texture_usages::PyTextureUsages;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::texture_usage_rules::check_main_texture_usages;

/// MCP reflected writes bypass the setter.
fn validate_main_texture_usages(usages: &CameraMainTextureUsages) -> Result<(), String> {
    check_main_texture_usages(usages.0).map(drop)
}

fn validate_usages_field(usages: &TextureUsages) -> Result<(), StorageError> {
    check_main_texture_usages(*usages)
        .map(drop)
        .map_err(StorageError::InvalidValue)
}

fn checked(usages: PyTextureUsages) -> PyResult<TextureUsages> {
    check_main_texture_usages(usages.try_into()?).map_err(PyValueError::new_err)
}

#[pycomponent(CameraMainTextureUsages, bridge, reflect_validate = validate_main_texture_usages)]
#[pyclass(name = "CameraMainTextureUsages", module = "pybevy.camera", extends = PyComponent, skip_from_py_object)]
pub struct PyCameraMainTextureUsages {
    storage: ComponentStorage<CameraMainTextureUsages>,
}

#[pymethods]
impl PyCameraMainTextureUsages {
    #[new]
    #[pyo3(signature = (value = CameraMainTextureUsages::default().0.into()))]
    pub fn new(value: PyTextureUsages) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(CameraMainTextureUsages(checked(value)?)).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyTextureUsages> {
        Ok(self.storage.borrow_validated_field_as(
            |usages| &usages.0,
            |usages| &mut usages.0,
            validate_usages_field,
        )?)
    }

    #[setter]
    pub fn set_value(&mut self, value: PyTextureUsages) -> PyResult<()> {
        let value = checked(value)?;
        self.as_mut()?.0 = value;
        Ok(())
    }

    #[pyo3(name = "with_")]
    pub fn with_(&self, py: Python<'_>, usages: PyTextureUsages) -> PyResult<Py<Self>> {
        let combined = self.as_ref()?.0 | TextureUsages::try_from(usages)?;
        let combined = check_main_texture_usages(combined).map_err(PyValueError::new_err)?;
        Py::new(py, Self::from_owned(CameraMainTextureUsages(combined)))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "CameraMainTextureUsages(value={})",
            self.value()?.__repr__()?
        ))
    }
}
