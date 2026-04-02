use bevy::prelude::ViewVisibility;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(ViewVisibility, bridge)]
#[pyclass(name = "ViewVisibility", extends = PyComponent, frozen)]
#[derive(Debug, Clone)]
pub struct PyViewVisibility {
    pub(crate) storage: ComponentStorage<ViewVisibility>,
}

#[pymethods]
impl PyViewVisibility {
    pub fn get(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.get())
    }

    #[classattr]
    #[pyo3(name = "HIDDEN")]
    pub fn hidden(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (ViewVisibility::HIDDEN.into(), PyComponent))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ViewVisibility({})", self.as_ref()?.get()))
    }

    pub fn __bool__(&self) -> PyResult<bool> {
        self.get()
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
