use bevy::ui::UiTargetCamera;
use pybevy_core::{ComponentStorage, PyComponent, PyEntity};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(UiTargetCamera, bridge)]
#[pyclass(name = "UiTargetCamera", extends = PyComponent, eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyUiTargetCamera {
    pub(crate) storage: ComponentStorage<UiTargetCamera>,
}

#[pymethods]
impl PyUiTargetCamera {
    #[new]
    pub fn new(entity: PyEntity) -> (Self, PyComponent) {
        Self::from_owned(UiTargetCamera(entity.into()))
    }

    #[getter]
    pub fn entity(&self) -> PyResult<PyEntity> {
        Ok(self.as_ref()?.entity().into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("UiTargetCamera({:?})", self.as_ref()?.entity()))
    }
}
