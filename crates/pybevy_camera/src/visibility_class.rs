use bevy::camera::visibility::VisibilityClass;
use pybevy_core::{PyComponent, component_storage::ComponentStorage, registry::global_registry};
use pybevy_macros::component_storage;
use pyo3::{prelude::*, types::PyType};

#[component_storage(VisibilityClass)]
#[pyclass(name = "VisibilityClass", extends = PyComponent)]
#[derive(Clone)]
pub struct PyVisibilityClass {
    pub(crate) storage: ComponentStorage<VisibilityClass>,
}

#[pymethods]
impl PyVisibilityClass {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (
            PyVisibilityClass {
                storage: ComponentStorage::owned(VisibilityClass::default()),
            },
            PyComponent,
        )
    }

    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.len())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_empty())
    }

    pub fn contains(&self, _py: Python<'_>, component_type: &Bound<'_, PyType>) -> PyResult<bool> {
        let ptr = component_type.as_type_ptr();
        let type_id = global_registry::get_type_id_by_py_type(ptr).ok_or_else(|| {
            let name = component_type
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "<unknown>".to_string());
            pyo3::exceptions::PyTypeError::new_err(format!(
                "Type '{}' is not a registered component type",
                name
            ))
        })?;
        Ok(self.as_ref()?.contains(&type_id))
    }

    pub fn add(&mut self, _py: Python<'_>, component_type: &Bound<'_, PyType>) -> PyResult<()> {
        let ptr = component_type.as_type_ptr();
        let type_id = global_registry::get_type_id_by_py_type(ptr).ok_or_else(|| {
            let name = component_type
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "<unknown>".to_string());
            pyo3::exceptions::PyTypeError::new_err(format!(
                "Type '{}' is not a registered component type",
                name
            ))
        })?;
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
