use bevy::camera::visibility::Visibility;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

use crate::visibility_batch::PyVisibilityBatch;

#[component_storage(Visibility)]
#[pyclass(name = "Visibility", extends = PyComponent, eq)]
#[derive(Debug, Clone)]
pub struct PyVisibility {
    pub(crate) storage: ComponentStorage<Visibility>,
}

impl PartialEq for PyVisibility {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyVisibility {
    #[staticmethod]
    #[pyo3(name = "INHERITED")]
    pub fn inherited(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Visibility::Inherited.into(), PyComponent))
    }

    #[staticmethod]
    #[pyo3(name = "VISIBLE")]
    pub fn visible(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Visibility::Visible.into(), PyComponent))
    }

    #[staticmethod]
    #[pyo3(name = "HIDDEN")]
    pub fn hidden(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Visibility::Hidden.into(), PyComponent))
    }

    #[staticmethod]
    pub fn from_numpy(py: Python, visibility: Py<PyAny>) -> PyResult<Py<PyAny>> {
        let batch = PyVisibilityBatch::new(visibility);
        Ok(Py::new(py, batch)?.into_any())
    }

    #[new]
    pub fn new() -> (Self, PyComponent) {
        (Visibility::Inherited.into(), PyComponent)
    }

    pub fn is_visible(&self) -> PyResult<bool> {
        Ok(*self.as_ref()? == Visibility::Visible)
    }

    pub fn is_hidden(&self) -> PyResult<bool> {
        Ok(*self.as_ref()? == Visibility::Hidden)
    }

    pub fn is_inherited(&self) -> PyResult<bool> {
        Ok(*self.as_ref()? == Visibility::Inherited)
    }

    pub fn toggle_inherited_visible(&mut self) -> PyResult<()> {
        self.as_mut()?.toggle_inherited_visible();
        Ok(())
    }

    pub fn toggle_inherited_hidden(&mut self) -> PyResult<()> {
        self.as_mut()?.toggle_inherited_hidden();
        Ok(())
    }

    pub fn toggle_visible_hidden(&mut self) -> PyResult<()> {
        self.as_mut()?.toggle_visible_hidden();
        Ok(())
    }

    pub fn set_visible(&mut self) -> PyResult<()> {
        *self.as_mut()? = Visibility::Visible;
        Ok(())
    }

    pub fn set_hidden(&mut self) -> PyResult<()> {
        *self.as_mut()? = Visibility::Hidden;
        Ok(())
    }

    pub fn set_inherited(&mut self) -> PyResult<()> {
        *self.as_mut()? = Visibility::Inherited;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("Visibility({:?})", self.as_ref()?))
    }
}
