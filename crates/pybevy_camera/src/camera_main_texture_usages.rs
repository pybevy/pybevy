use bevy::camera::CameraMainTextureUsages;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_render::texture_usages::PyTextureUsages;
use pyo3::prelude::*;

#[pycomponent(CameraMainTextureUsages, bridge)]
#[pyclass(name = "CameraMainTextureUsages", module = "pybevy.camera", extends = PyComponent, skip_from_py_object)]
pub struct PyCameraMainTextureUsages {
    storage: ComponentStorage<CameraMainTextureUsages>,
}

#[pymethods]
impl PyCameraMainTextureUsages {
    #[new]
    #[pyo3(signature = (value = CameraMainTextureUsages::default().0.into()))]
    pub fn new(value: PyTextureUsages) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(CameraMainTextureUsages(value.try_into()?)).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyTextureUsages> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|usages| &usages.0, |usages| &mut usages.0)?)
    }

    #[setter]
    pub fn set_value(&mut self, value: PyTextureUsages) -> PyResult<()> {
        let value = value.try_into()?;
        self.as_mut()?.0 = value;
        Ok(())
    }

    #[pyo3(name = "with_")]
    pub fn with_(&self, py: Python<'_>, usages: PyTextureUsages) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(self.as_ref()?.clone().with(usages.try_into()?)),
        )
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "CameraMainTextureUsages(value={})",
            self.value()?.__repr__()?
        ))
    }
}
