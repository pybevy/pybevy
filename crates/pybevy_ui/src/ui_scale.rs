use bevy::ui::UiScale;
use pybevy_core::{
    PyResource, ResourceStorage, public_error, repr::float_repr, resource_initializer,
};
use pybevy_macros::pyresource;
use pyo3::{exceptions::PyValueError, prelude::*};

fn validate_ui_scale(scale: f32) -> PyResult<f32> {
    if !scale.is_finite() {
        return Err(PyValueError::new_err(public_error::ui_scale_not_finite(
            scale,
        )));
    }
    if scale <= 0.0 {
        return Err(PyValueError::new_err(public_error::ui_scale_not_positive(
            scale,
        )));
    }
    Ok(scale)
}

fn clone_ui_scale(value: &UiScale) -> PyResult<UiScale> {
    Ok(UiScale(value.0))
}

#[pyresource(UiScale, no_clone, bridge, clone_with = clone_ui_scale)]
#[pyclass(name = "UiScale", module = "pybevy.ui", extends = PyResource)]
#[derive(Debug)]
pub struct PyUiScale {
    pub(crate) storage: ResourceStorage<UiScale>,
}

#[pymethods]
impl PyUiScale {
    #[new]
    #[pyo3(signature = (scale = 1.0))]
    pub fn new(scale: f32) -> PyResult<PyClassInitializer<Self>> {
        Ok(resource_initializer(Self {
            storage: ResourceStorage::owned(UiScale(validate_ui_scale(scale)?)),
        }))
    }

    #[getter]
    pub fn scale(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_scale(&mut self, value: f32) -> PyResult<()> {
        let value = validate_ui_scale(value)?;
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(s) => format!("UiScale({})", float_repr(s.0)),
            Err(_) => "UiScale(<invalid>)".to_string(),
        }
    }
}
