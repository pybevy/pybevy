//! Base resource class for PyBevy
//!
//! This module provides the base `PyResource` marker class that all
//! PyBevy resource types extend.

use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};

/// Base class for all PyBevy resources
///
/// This is a marker class that provides the Python type hierarchy.
/// All resource types (GlobalVolume, Time, etc.) extend this class.
#[pyclass(name = "Resource", subclass, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyResource;

#[pymethods]
impl PyResource {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        PyResource
    }
}
