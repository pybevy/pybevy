//! Asset base class and trait for PyBevy asset wrappers
//!
//! This module provides:
//! - `PyAsset`: Base Python class for all asset wrappers
//! - `NativeAsset`: Trait for accessing the underlying Bevy asset

use bevy::asset::Asset;
use pyo3::prelude::*;

/// Base class for all PyBevy asset wrappers
///
/// All asset types (Mesh, StandardMaterial, AudioSource, etc.) extend this class
/// to provide a common interface and enable isinstance() checks.
#[pyclass(name = "Asset", subclass)]
#[derive(Debug, Clone)]
pub struct PyAsset;

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
