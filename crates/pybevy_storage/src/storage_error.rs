//! PyO3-independent error types for storage operations
//!
//! This module defines `StorageError`, a plain Rust error enum that all storage
//! types use for their fallible operations. A `From<StorageError> for PyErr` impl
//! allows the `?` operator to auto-convert in `#[pymethods]` blocks that return
//! `PyResult`, keeping call sites unchanged while decoupling the storage layer
//! from PyO3.

use std::fmt;

/// Error message for accessing components outside system execution
const ERR_OUTSIDE_SYSTEM: &str = "PyBevy component accessed outside of system execution. \
     Query parameters are only valid during the system's execution. \
     Do not store them in global variables or use them after the system has finished.";

/// Errors returned by storage operations.
///
/// Each variant maps to a specific Python exception via `From<StorageError> for PyErr`.
#[derive(Debug, Clone)]
pub enum StorageError {
    /// Accessed outside system execution (`RuntimeError`)
    InvalidAccess,

    /// A re-resolving borrow's entity was despawned or its component removed
    /// (`RuntimeError`).
    EntityUnavailable,

    /// Write on read-only component (`RuntimeError`)
    ReadOnly,

    /// Asset already consumed by `Assets<T>.add()` (`RuntimeError`)
    AssetConsumed,

    /// Can't take ownership of borrowed asset (`RuntimeError`)
    AssetBorrowed,

    /// Can't modify read-only asset (`RuntimeError`)
    AssetReadOnly,

    /// List index out of range (`IndexError`)
    IndexOutOfRange,

    /// Mutation on a field extracted from an owned/temporary component (`RuntimeError`)
    OwnedFieldReadOnly,

    /// Pop from empty list (`IndexError`)
    EmptyList,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::InvalidAccess => f.write_str(ERR_OUTSIDE_SYSTEM),
            StorageError::EntityUnavailable => f.write_str(
                "Component no longer available (entity despawned or component removed).",
            ),
            StorageError::ReadOnly => f.write_str(
                "Cannot modify read-only component. \
                 Use Query[Mut[ComponentType]] instead of Query[ComponentType] for mutable access.",
            ),
            StorageError::AssetConsumed => f.write_str(
                "Asset was already consumed (added to Assets<T>). \
                 Create a new asset instance instead.",
            ),
            StorageError::AssetBorrowed => f.write_str(
                "Cannot take ownership of borrowed asset. \
                 Only owned assets can be consumed.",
            ),
            StorageError::AssetReadOnly => f.write_str(
                "Cannot modify asset obtained from Res[Assets[T]].get(). \
                 Use ResMut[Assets[T]].get_mut() for mutable access.",
            ),
            StorageError::OwnedFieldReadOnly => f.write_str(
                "Cannot mutate a field extracted from an owned or temporary component. \
                 Assign the field back through the component, \
                 e.g. `transform.translation = Vec3(...)` instead of `transform.translation.x = 5.0`.",
            ),
            StorageError::IndexOutOfRange => f.write_str("list index out of range"),
            StorageError::EmptyList => f.write_str("pop from empty list"),
        }
    }
}

impl std::error::Error for StorageError {}

#[cfg(feature = "pyo3")]
impl From<StorageError> for pyo3::PyErr {
    fn from(err: StorageError) -> Self {
        use pyo3::exceptions::{PyIndexError, PyRuntimeError};
        match err {
            StorageError::InvalidAccess
            | StorageError::EntityUnavailable
            | StorageError::ReadOnly
            | StorageError::OwnedFieldReadOnly
            | StorageError::AssetConsumed
            | StorageError::AssetBorrowed
            | StorageError::AssetReadOnly => PyRuntimeError::new_err(err.to_string()),
            StorageError::IndexOutOfRange | StorageError::EmptyList => {
                PyIndexError::new_err(err.to_string())
            }
        }
    }
}
