//! Base component class for PyBevy
//!
//! This module provides the base `PyComponent` marker class that all
//! PyBevy component types extend.

use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};

/// Base class for all PyBevy components
///
/// This is a marker class that provides the Python type hierarchy.
/// All component types (Transform, PointLight, etc.) extend this class.
#[pyclass(name = "Component", subclass, frozen, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyComponent;

#[pymethods]
impl PyComponent {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        PyComponent
    }
}
