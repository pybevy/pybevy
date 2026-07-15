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

/// Error message for accessing a system parameter from the wrong thread
const ERR_CROSS_THREAD: &str = "PyBevy system parameter used from a different thread than the one \
     executing the system. Query/View/World/Commands parameters are pinned to \
     the system's thread and must not be shared across threads: do not stash \
     them in a global read by another system running in parallel, or hand them \
     to a thread you spawned.";

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

    /// System parameter used from a thread other than the one executing the
    /// system (`RuntimeError`). Rejected before any pointer dereference, so a
    /// wrapper shared to a Python-spawned thread or to a concurrently scheduled
    /// system on a worker thread cannot cause a use-after-free / data race.
    CrossThreadAccess,

    /// Write on read-only component (`RuntimeError`)
    ReadOnly,

    /// Asset already consumed by `Assets<T>.add()` (`RuntimeError`)
    AssetConsumed,

    /// Can't take ownership of borrowed asset (`RuntimeError`)
    AssetBorrowed,

    /// Can't modify read-only asset (`RuntimeError`)
    AssetReadOnly,

    /// Borrowed asset or its `Assets<T>` collection disappeared during access
    /// (`RuntimeError`).
    AssetUnavailable,

    /// Operation would alias or invalidate a live NumPy view (`RuntimeError`)
    AssetViewsLive,

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
            StorageError::CrossThreadAccess => f.write_str(ERR_CROSS_THREAD),
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
            StorageError::AssetUnavailable => {
                f.write_str("Borrowed asset is no longer available in Assets<T>.")
            }
            StorageError::AssetViewsLive => f.write_str(
                "A live NumPy view still aliases this asset's data. \
                 Drop the array (del it or leave the with-block that created it) \
                 before mutating or consuming the asset.",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_only_error_suggests_mut() {
        let err = StorageError::ReadOnly;
        assert!(err.to_string().contains("Mut["));
    }

    #[test]
    fn test_invalid_access_says_outside_system() {
        let err = StorageError::InvalidAccess;
        assert!(err.to_string().contains("outside of system execution"));
    }

    #[test]
    fn test_entity_unavailable_message() {
        let err = StorageError::EntityUnavailable;
        assert!(err.to_string().contains("despawned"));
    }

    #[test]
    fn test_asset_consumed_suggests_new_instance() {
        let err = StorageError::AssetConsumed;
        assert!(err.to_string().contains("Create a new asset instance"));
    }

    #[test]
    fn test_asset_read_only_suggests_get_mut() {
        let err = StorageError::AssetReadOnly;
        assert!(err.to_string().contains("get_mut()"));
    }

    #[test]
    fn test_asset_unavailable_names_asset_collection() {
        let err = StorageError::AssetUnavailable;
        assert!(err.to_string().contains("Assets<T>"));
    }

    #[test]
    fn test_owned_field_read_only_suggests_assign_back() {
        let err = StorageError::OwnedFieldReadOnly;
        assert!(err.to_string().contains("Assign the field back"));
    }

    #[test]
    fn test_index_out_of_range_message() {
        let err = StorageError::IndexOutOfRange;
        assert_eq!(err.to_string(), "list index out of range");
    }

    #[test]
    fn test_empty_list_message() {
        let err = StorageError::EmptyList;
        assert_eq!(err.to_string(), "pop from empty list");
    }
}

#[cfg(feature = "pyo3")]
impl From<StorageError> for pyo3::PyErr {
    fn from(err: StorageError) -> Self {
        use pyo3::exceptions::{PyIndexError, PyRuntimeError};
        match err {
            StorageError::InvalidAccess
            | StorageError::EntityUnavailable
            | StorageError::CrossThreadAccess
            | StorageError::ReadOnly
            | StorageError::OwnedFieldReadOnly
            | StorageError::AssetConsumed
            | StorageError::AssetBorrowed
            | StorageError::AssetReadOnly
            | StorageError::AssetUnavailable
            | StorageError::AssetViewsLive => PyRuntimeError::new_err(err.to_string()),
            StorageError::IndexOutOfRange | StorageError::EmptyList => {
                PyIndexError::new_err(err.to_string())
            }
        }
    }
}
