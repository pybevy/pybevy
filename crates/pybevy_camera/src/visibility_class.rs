use std::any::TypeId;

use bevy::camera::visibility::VisibilityClass;
use pybevy_core::{
    PyComponent, public_error::unregistered_component_type, pycomponent::ComponentStorage,
    registry::global_registry,
};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyType};

fn registered_component_type_id(component_type: &Bound<'_, PyType>) -> PyResult<TypeId> {
    global_registry::get_type_id_by_py_type(component_type.as_type_ptr()).ok_or_else(|| {
        let name = component_type
            .name()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "<unknown>".to_string());
        PyTypeError::new_err(unregistered_component_type(&name))
    })
}

#[pycomponent(VisibilityClass, bridge)]
#[pyclass(name = "VisibilityClass", module = "pybevy.camera", extends = PyComponent)]
pub struct PyVisibilityClass {
    pub(crate) storage: ComponentStorage<VisibilityClass>,
}

#[pymethods]
impl PyVisibilityClass {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (
            PyVisibilityClass {
                storage: ComponentStorage::owned(VisibilityClass::default()),
            },
            PyComponent,
        )
            .into()
    }

    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.len())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_empty())
    }

    pub fn contains(&self, _py: Python<'_>, component_type: &Bound<'_, PyType>) -> PyResult<bool> {
        let type_id = registered_component_type_id(component_type)?;
        Ok(self.as_ref()?.contains(&type_id))
    }

    pub fn add(&mut self, _py: Python<'_>, component_type: &Bound<'_, PyType>) -> PyResult<()> {
        let type_id = registered_component_type_id(component_type)?;
        self.as_mut()?.push(type_id);
        Ok(())
    }

    pub fn clear(&mut self) -> PyResult<()> {
        self.as_mut()?.clear();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let count = self.as_ref()?.len();
        Ok(format!("VisibilityClass({} classes)", count))
    }
}
