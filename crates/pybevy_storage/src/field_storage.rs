//! Generic field storage supporting both owned and borrowed instances
//!
//! This module provides storage for non-Copy field types (like TextureAtlas)
//! that can be accessed from component fields.
//!
//! Key difference from ValueStorage:
//! - FieldStorage is for non-Copy types (uses Box for owned values)
//! - ValueStorage is for Copy types (stores directly)
//!
//! # Safety Model
//!
//! FieldStorage follows the same safety model as ComponentStorage:
//!
//! ## Owned Mode
//!
//! - Data is stored in `Box<T>` with a `ValidityFlag` for field borrow tracking
//! - Field borrows share the parent's validity flag
//! - `Drop` invalidates the flag before the Box is dropped, preventing use-after-free
//!
//! ## Borrowed Mode
//!
//! - Raw pointer into parent storage (component or another FieldStorage)
//! - `ValidityFlagWithMode` tracks validity and read/write permission
//! - Inherits validity from parent (invalidated when parent's system exits)

use crate::{
    storage_error::StorageError,
    storage_traits::{BorrowableStorage, FromBorrowedStorage},
    validity_guard::{ValidityFlag, ValidityFlagWithMode},
};

/// Generic storage for PyBevy field types (non-Copy types like TextureAtlas)
///
/// Supports two modes:
/// - `Owned`: Python-created value, stored in Box
/// - `Borrowed`: Reference to field in a component (e.g., sprite.texture_atlas)
///
/// # Type Parameters
/// - `T`: The field type (does not need to implement `Copy`)
///
/// # Safety
/// Borrowed variant contains a raw pointer to value data in a component.
/// The `ValidityFlagWithMode` ensures this pointer is only dereferenced during
/// system execution when the pointer is guaranteed to be valid.
#[derive(Debug)]
pub struct FieldStorage<T: Clone> {
    pub inner: FieldStorageInner<T>,
}

#[derive(Debug)]
pub enum FieldStorageInner<T: Clone> {
    /// Python-created value, stored in Box with validity tracking
    ///
    /// The ValidityFlag ensures that field borrows (raw pointers into the Box)
    /// cannot be used after the FieldStorage is dropped.
    Owned {
        /// Heap-allocated field data
        data: Box<T>,

        /// Validity tracking for field borrows
        /// Invalidated when FieldStorage is dropped
        validity: ValidityFlag,
    },

    /// Read-only snapshot of a field extracted from owned/temporary storage.
    /// Reads succeed; writes return `StorageError::OwnedFieldReadOnly`.
    OwnedReadOnly {
        /// Heap-allocated field data (clone of the original)
        data: Box<T>,
    },

    /// Borrowed reference to field in a component
    Borrowed {
        /// Pointer to value in component field
        ptr: *mut T,

        /// Validity tracking with read/write mode
        validity: ValidityFlagWithMode,
    },
}

// SAFETY: FieldStorage is Send because:
// - Box<T> is Send when T is Send
// - Raw pointer is just an address
// - ValidityFlagWithMode is Send + Sync
// - Validity checking prevents unsafe access
unsafe impl<T: Clone + Send> Send for FieldStorage<T> {}

// SAFETY: FieldStorage is Sync because:
// - Access is controlled by validity checking
// - ValidityFlagWithMode uses atomic operations
unsafe impl<T: Clone + Sync> Sync for FieldStorage<T> {}

impl<T: Clone> Clone for FieldStorage<T> {
    fn clone(&self) -> Self {
        match &self.inner {
            FieldStorageInner::Owned { data, validity: _ } => {
                // CRITICAL: Create a NEW validity flag for the clone.
                // Each owned instance needs independent validity tracking.
                Self {
                    inner: FieldStorageInner::Owned {
                        data: Box::new((**data).clone()),
                        validity: ValidityFlag::new_write(),
                    },
                }
            }
            FieldStorageInner::OwnedReadOnly { data } => Self {
                inner: FieldStorageInner::OwnedReadOnly {
                    data: Box::new((**data).clone()),
                },
            },
            FieldStorageInner::Borrowed { ptr, validity } => Self {
                inner: FieldStorageInner::Borrowed {
                    ptr: *ptr,
                    validity: validity.clone(),
                },
            },
        }
    }
}

impl<T: Clone + PartialEq> PartialEq for FieldStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (
                FieldStorageInner::Owned { data: a, .. },
                FieldStorageInner::Owned { data: b, .. },
            ) => **a == **b,
            (
                FieldStorageInner::OwnedReadOnly { data: a },
                FieldStorageInner::OwnedReadOnly { data: b },
            ) => **a == **b,
            (
                FieldStorageInner::Borrowed { ptr: a, .. },
                FieldStorageInner::Borrowed { ptr: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

impl<T: Clone> Drop for FieldStorage<T> {
    fn drop(&mut self) {
        // Invalidate owned storage's validity flag before the Box is dropped.
        // This ensures any outstanding field borrows will fail their validity checks.
        //
        // For borrowed storage, the validity is managed by the parent (component or
        // another FieldStorage), so we don't invalidate here.
        if let FieldStorageInner::Owned { validity, .. } = &self.inner {
            validity.set_invalid();
        }
    }
}

impl<T: Clone> BorrowableStorage<T> for FieldStorage<T> {
    unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self {
        Self {
            inner: FieldStorageInner::Borrowed { ptr, validity },
        }
    }

    fn snapshot(value: &T) -> Self {
        Self {
            inner: FieldStorageInner::OwnedReadOnly {
                data: Box::new(value.clone()),
            },
        }
    }
}

impl<T: Clone> FieldStorage<T> {
    /// Create owned field storage with validity tracking
    pub fn owned(value: T) -> Self {
        Self {
            inner: FieldStorageInner::Owned {
                data: Box::new(value),
                validity: ValidityFlag::new_write(),
            },
        }
    }

    /// Get immutable reference to the value, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, StorageError> {
        self.check_valid()?;
        Ok(unsafe { &*self.as_ptr() })
    }

    /// Get mutable reference to the value, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        self.check_valid_mut()?;
        Ok(unsafe { &mut *self.as_mut_ptr() })
    }

    /// Get raw const pointer to the value
    #[inline(always)]
    fn as_ptr(&self) -> *const T {
        match &self.inner {
            FieldStorageInner::Owned { data, .. } | FieldStorageInner::OwnedReadOnly { data } => {
                &**data as *const T
            }
            FieldStorageInner::Borrowed { ptr, .. } => *ptr as *const T,
        }
    }

    /// Get raw mutable pointer to the value
    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut T {
        match &mut self.inner {
            FieldStorageInner::Owned { data, .. } | FieldStorageInner::OwnedReadOnly { data } => {
                &mut **data as *mut T
            }
            FieldStorageInner::Borrowed { ptr, .. } => *ptr,
        }
    }

    /// Check if this value reference is still valid for reading
    #[inline(always)]
    fn check_valid(&self) -> Result<(), StorageError> {
        match &self.inner {
            FieldStorageInner::Owned { .. } | FieldStorageInner::OwnedReadOnly { .. } => Ok(()),
            FieldStorageInner::Borrowed { validity, .. } => validity.check(),
        }
    }

    /// Check if this value reference is still valid for writing
    #[inline(always)]
    fn check_valid_mut(&self) -> Result<(), StorageError> {
        match &self.inner {
            FieldStorageInner::Owned { .. } => Ok(()),
            FieldStorageInner::OwnedReadOnly { .. } => Err(StorageError::OwnedFieldReadOnly),
            FieldStorageInner::Borrowed { validity, .. } => validity.check_write(),
        }
    }

    /// Get the current value (returns a clone)
    #[inline(always)]
    pub fn get(&self) -> Result<T, StorageError> {
        Ok(self.as_ref()?.clone())
    }

    /// Borrow a field from the stored value
    ///
    /// Returns a borrowed reference to a nested field that can be mutated
    /// and have changes persist back to the original storage.
    ///
    /// # Example
    ///
    /// Prefer using `borrow_field_as` for simpler syntax:
    /// ```rust,ignore
    /// #[getter]
    /// pub fn physical_position(&self) -> PyResult<PyUVec2> {
    ///     self.storage.borrow_field_as(|v| &v.physical_position)
    /// }
    /// ```
    ///
    /// Or use `borrow_field` directly when more control is needed:
    /// ```rust,ignore
    /// let storage = self.storage.borrow_field(|v| &v.physical_position)?;
    /// Ok(PyUVec2::from_borrowed(storage))
    /// ```
    pub fn borrow_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        match &self.inner {
            FieldStorageInner::Owned { data, .. }
            | FieldStorageInner::OwnedReadOnly { data, .. } => {
                Ok(S::snapshot(field_accessor(&**data)))
            }
            FieldStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                // SAFETY: We just checked validity above. The ptr points to valid storage
                // that remains stable during system execution.
                let value_ref = unsafe { &**ptr };
                let field_ref = field_accessor(value_ref);
                let field_ptr = field_ref as *const F as *mut F;
                // SAFETY: field_ptr points into the parent storage at a stable offset.
                // The validity flag from the parent ensures this remains valid.
                Ok(unsafe { S::borrowed(field_ptr, validity.clone()) })
            }
        }
    }

    /// Helper to borrow a field and wrap it in the final Python type
    ///
    /// Combines `borrow_field` with `FromBorrowedStorage::from_borrowed` to reduce boilerplate.
    ///
    /// # Example
    /// ```rust,ignore
    /// #[getter]
    /// pub fn physical_position(&self) -> PyResult<PyUVec2> {
    ///     self.storage.borrow_field_as(|v| &v.physical_position)
    /// }
    /// ```
    pub fn borrow_field_as<F: Clone, S, W>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        Ok(W::from_borrowed(self.borrow_field(field_accessor)?))
    }
}
