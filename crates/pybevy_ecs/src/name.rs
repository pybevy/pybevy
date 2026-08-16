use bevy::ecs::name::Name;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Name, bridge)]
#[pyclass(name = "Name", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyName {
    pub(crate) storage: ComponentStorage<Name>,
}

#[pymethods]
impl PyName {
    #[new]
    #[pyo3(signature = (name = String::new()))]
    pub fn new(name: String) -> PyClassInitializer<Self> {
        Self::from_owned(Name::new(name)).into()
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.as_ref()?.as_str().to_string())
    }

    #[setter]
    pub fn set_name(&mut self, name: String) -> PyResult<()> {
        self.as_mut()?.set(name);
        Ok(())
    }

    pub fn as_str(&self) -> PyResult<String> {
        self.name()
    }

    fn __repr__(&self) -> PyResult<String> {
        let value = self.as_ref()?;
        let name = value.as_str();
        Ok(format!("Name(\"{}\")", name))
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(self.as_ref()?.as_str().to_string())
    }
}

impl TryFrom<&PyName> for Name {
    type Error = PyErr;

    fn try_from(py_name: &PyName) -> Result<Self, Self::Error> {
        Ok(py_name.as_ref()?.clone())
    }
}
