//! Asset base class and trait for PyBevy asset wrappers
//!
//! This module provides:
//! - `PyAsset`: Base Python class for all asset wrappers
//! - `NativeAsset`: Trait for accessing the underlying Bevy asset

use bevy::asset::Asset;
use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};

/// Base class for all PyBevy asset wrappers
///
/// All asset types (Mesh, StandardMaterial, AudioSource, etc.) extend this class
/// to provide a common interface and enable isinstance() checks.
#[pyclass(name = "Asset", subclass, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAsset;

/// Base class for `@material` classes, so `Assets[T]` accepts them.
#[pyclass(name = "Material", extends = PyAsset, subclass, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMaterial;

#[pymethods]
impl PyMaterial {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(
        _args: &Bound<'_, PyTuple>,
        _kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyAsset).add_subclass(PyMaterial)
    }
}

/// Trait for PyBevy asset wrappers
///
/// This trait provides access to the underlying Bevy asset type and allows
/// consuming the asset when adding it to `Assets<T>` storage.
pub trait NativeAsset {
    /// The Bevy asset type this wrapper represents
    type Asset: Asset;

    /// Take ownership of the asset, consuming it from storage
    ///
    /// This method is implemented by all PyBevy asset wrappers and allows
    /// moving the asset out of Python into Bevy's Assets<T> storage.
    ///
    /// # Errors
    /// Returns error if asset was already consumed or is borrowed
    fn take(&mut self) -> PyResult<Self::Asset>;
}
