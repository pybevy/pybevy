use bevy::prelude::InheritedVisibility;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(InheritedVisibility, bridge)]
#[pyclass(name = "InheritedVisibility", module = "pybevy.camera", extends = PyComponent, frozen)]
#[derive(Debug)]
pub struct PyInheritedVisibility {
    pub(crate) storage: ComponentStorage<InheritedVisibility>,
}

#[pymethods]
impl PyInheritedVisibility {
    pub fn get(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.get())
    }

    #[classattr]
    #[pyo3(name = "VISIBLE")]
    pub fn visible(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (InheritedVisibility::VISIBLE.into(), PyComponent))
    }

    #[classattr]
    #[pyo3(name = "HIDDEN")]
    pub fn hidden(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (InheritedVisibility::HIDDEN.into(), PyComponent))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("InheritedVisibility({})", self.as_ref()?.get()))
    }

    pub fn __bool__(&self) -> PyResult<bool> {
        self.get()
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
