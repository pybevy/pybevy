use bevy::pbr::wireframe::WireframeXray;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(WireframeXray, bridge, view_fields = [0 as value])]
#[pyclass(name = "WireframeXray", module = "pybevy.pbr", extends = PyComponent)]
#[derive(Debug)]
pub struct PyWireframeXray {
    pub(crate) storage: ComponentStorage<WireframeXray>,
}

#[pymethods]
impl PyWireframeXray {
    #[new]
    #[pyo3(signature = (value = false))]
    pub fn new(value: bool) -> PyClassInitializer<Self> {
        Self::from_owned(WireframeXray(value)).into()
    }

    #[getter]
    pub fn value(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "WireframeXray({})",
            if self.as_ref()?.0 { "True" } else { "False" }
        ))
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()?.0 == other.as_ref()?.0)
    }
}
