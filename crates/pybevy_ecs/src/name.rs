//! Name component for entity identification.

use bevy::ecs::name::Name;
use pybevy_core::{ComponentStorage, PyComponent};
use pyo3::prelude::*;

/// Component used to identify an entity with a human-readable name.
///
/// Names are not unique - multiple entities can have the same name.
/// Use Entity for unique identification.
///
/// Example:
///     commands.spawn((Name("Player"), Transform()))
///
///     # Query by name
///     for name in query:
///         print(f"Entity name: {name}")
#[pyclass(name = "Name", extends = PyComponent, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyName {
    pub(crate) storage: ComponentStorage<Name>,
}

impl PyName {
    pub fn from_owned(value: Name) -> (Self, PyComponent) {
        (
            PyName {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }

    pub fn from_borrowed(storage: ComponentStorage<Name>) -> (Self, PyComponent) {
        (PyName { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&Name> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut Name> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyName {
    #[new]
    #[pyo3(signature = (name = String::new()))]
    pub fn new(name: String) -> (Self, PyComponent) {
        Self::from_owned(Name::new(name))
    }

    /// Get the name as a string.
    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.as_ref()?.as_str().to_string())
    }

    /// Set the name.
    #[setter]
    pub fn set_name(&mut self, name: String) -> PyResult<()> {
        self.as_mut()?.set(name);
        Ok(())
    }

    /// Get the name as a string (alias for name property).
    pub fn as_str(&self) -> PyResult<String> {
        self.name()
    }

    fn __repr__(&self) -> PyResult<String> {
        let name = self.as_ref()?.as_str();
        Ok(format!("Name(\"{}\")", name))
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(self.as_ref()?.as_str().to_string())
    }
}

// TryFrom for spawn conversion (Python -> Bevy)
impl TryFrom<&PyName> for Name {
    type Error = PyErr;

    fn try_from(py_name: &PyName) -> Result<Self, Self::Error> {
        Ok(py_name.as_ref()?.clone())
    }
}

// TryFrom for query extraction (Bevy -> Python)
impl TryFrom<&Name> for PyName {
    type Error = PyErr;

    fn try_from(name: &Name) -> Result<Self, Self::Error> {
        Ok(PyName {
            storage: ComponentStorage::owned(name.clone()),
        })
    }
}
