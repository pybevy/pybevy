//! Base resource class for PyBevy
//!
//! This module provides the base `PyResource` marker class that all
//! PyBevy resource types extend.

use pyo3::{
    PyClass,
    prelude::*,
    types::{PyDict, PyTuple},
};

use crate::PyComponent;

/// Base class for all PyBevy resources
///
/// This is a marker class that provides the Python type hierarchy.
/// All resource types (GlobalVolume, Time, etc.) extend this class.
#[pyclass(name = "Resource", extends = PyComponent, subclass, frozen, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyResource;

/// Build the `Component -> Resource -> T` initializer chain for a native
/// resource wrapper extending [`PyResource`].
pub fn resource_initializer<T>(value: T) -> PyClassInitializer<T>
where
    T: PyClass<BaseType = PyResource>,
{
    PyClassInitializer::from(PyComponent)
        .add_subclass(PyResource)
        .add_subclass(value)
}

#[pymethods]
impl PyResource {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(
        _args: &Bound<'_, PyTuple>,
        _kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyComponent).add_subclass(PyResource)
    }
}

#[cfg(test)]
mod tests {
    use pyo3::PyTypeInfo;

    use super::*;

    #[pyclass(extends = PyResource)]
    struct TestNativeResource;

    #[test]
    fn native_resource_initializer_builds_component_hierarchy() {
        Python::initialize();
        Python::attach(|py| {
            let resource_type = PyResource::type_object(py);
            let component_type = PyComponent::type_object(py);
            assert!(resource_type.is_subclass(&component_type).unwrap());

            let value = Py::new(py, resource_initializer(TestNativeResource)).unwrap();
            assert!(value.bind(py).is_instance_of::<PyResource>());
            assert!(value.bind(py).is_instance_of::<PyComponent>());
        });
    }
}
