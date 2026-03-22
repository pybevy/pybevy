//! Generic value storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for PyBevy value types
//! (Vec2, Vec3, Vec4, Quat, Mat3, Mat4, LinearRgba, etc.), eliminating code
//! duplication across math and color types.
//!
//! Key difference from ComponentStorage:
//! - ValueStorage stores Copy types directly (Owned(T))
//! - ComponentStorage stores larger types in Box (Owned(Box<T>))

use crate::{
    storage_error::StorageError,
    storage_traits::{BorrowableStorage, FromBorrowedStorage},
    validity_guard::ValidityFlagWithMode,
};

/// Generic storage for PyBevy value types (Copy types like Vec3, Quat, etc.)
///
/// Supports two modes:
/// - `Owned`: Python-created value, stored directly (no heap allocation)
/// - `Borrowed`: Reference to field in a component (e.g., transform.translation)
///
/// # Type Parameters
/// - `T`: The value type (must implement `Copy` for efficient storage)
///
/// # Safety
/// Borrowed variant contains a raw pointer to value data in a component.
/// The `ValidityFlagWithMode` ensures this pointer is only dereferenced during
/// system execution when the pointer is guaranteed to be valid, and tracks
/// whether the component was accessed mutably or immutably.
#[derive(Debug)]
pub struct ValueStorage<T: Copy> {
    pub inner: ValueStorageInner<T>,
}

#[derive(Debug)]
pub enum ValueStorageInner<T: Copy> {
    /// Python-created value, stored directly (no heap allocation needed)
    Owned(T),

    /// Read-only snapshot of a field extracted from owned/temporary storage.
    /// Reads succeed; writes return `StorageError::OwnedFieldReadOnly`.
    OwnedReadOnly(T),

    /// Borrowed reference to field in a component
    Borrowed {
        /// Pointer to value in component field
        ptr: *mut T,

        /// Validity tracking with read/write mode
        /// Prevents use after system execution and tracks mutability
        validity: ValidityFlagWithMode,
    },
}

// SAFETY: ValueStorage is Send because:
// - T is Copy, which implies Send for most numeric/simple types
// - Raw pointer is just an address
// - ValidityFlagWithMode (Arc<AtomicBool> + mode tracking) is Send + Sync
// - Validity checking prevents unsafe access
unsafe impl<T: Copy + Send> Send for ValueStorage<T> {}

// SAFETY: ValueStorage is Sync because:
// - Access is controlled by validity checking
// - ValidityFlagWithMode uses atomic operations
// - We only allow access when validity flag is true
unsafe impl<T: Copy + Sync> Sync for ValueStorage<T> {}

impl<T: Copy> Clone for ValueStorage<T> {
    fn clone(&self) -> Self {
        match &self.inner {
            ValueStorageInner::Owned(value) => Self {
                inner: ValueStorageInner::Owned(*value),
            },
            ValueStorageInner::OwnedReadOnly(value) => Self {
                inner: ValueStorageInner::OwnedReadOnly(*value),
            },
            ValueStorageInner::Borrowed { ptr, validity } => Self {
                inner: ValueStorageInner::Borrowed {
                    ptr: *ptr,
                    validity: validity.clone(),
                },
            },
        }
    }
}

impl<T: Copy> PartialEq for ValueStorage<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (ValueStorageInner::Owned(a), ValueStorageInner::Owned(b)) => a == b,
            (ValueStorageInner::OwnedReadOnly(a), ValueStorageInner::OwnedReadOnly(b)) => a == b,
            (
                ValueStorageInner::Borrowed { ptr: a, .. },
                ValueStorageInner::Borrowed { ptr: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

impl<T: Copy> BorrowableStorage<T> for ValueStorage<T> {
    unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self {
        Self {
            inner: ValueStorageInner::Borrowed { ptr, validity },
        }
    }

    fn snapshot(value: &T) -> Self {
        Self {
            inner: ValueStorageInner::OwnedReadOnly(*value),
        }
    }
}

impl<T: Copy> ValueStorage<T> {
    /// Create owned value storage
    pub const fn owned(value: T) -> Self {
        Self {
            inner: ValueStorageInner::Owned(value),
        }
    }

    /// Get immutable reference to the value, checking validity
    ///
    /// # Errors
    /// Returns `StorageError::InvalidAccess` if the borrowed reference is no longer valid
    /// (i.e., accessed outside of system execution context)
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, StorageError> {
        self.check_valid()?;
        Ok(unsafe { &*self.as_ptr() })
    }

    /// Get mutable reference to the value, checking validity and write permission
    ///
    /// # Errors
    /// Returns `StorageError` if:
    /// - The borrowed reference is no longer valid
    /// - The value was borrowed immutably (Ref) but mutable access is attempted
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        self.check_valid_mut()?;
        Ok(unsafe { &mut *self.as_mut_ptr() })
    }

    /// Get raw const pointer to the value
    ///
    /// # Safety
    /// Caller must ensure validity before dereferencing
    #[inline(always)]
    fn as_ptr(&self) -> *const T {
        match &self.inner {
            ValueStorageInner::Owned(value) | ValueStorageInner::OwnedReadOnly(value) => {
                value as *const T
            }
            ValueStorageInner::Borrowed { ptr, .. } => *ptr as *const T,
        }
    }

    /// Get raw mutable pointer to the value
    ///
    /// # Safety
    /// Caller must ensure validity and write permission before dereferencing
    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut T {
        match &mut self.inner {
            ValueStorageInner::Owned(value) | ValueStorageInner::OwnedReadOnly(value) => {
                value as *mut T
            }
            ValueStorageInner::Borrowed { ptr, .. } => *ptr,
        }
    }

    /// Check if this value reference is still valid for reading
    ///
    /// For owned values (including read-only snapshots), always returns `Ok(())`.
    /// For borrowed values, checks the validity flag.
    #[inline(always)]
    fn check_valid(&self) -> Result<(), StorageError> {
        match &self.inner {
            ValueStorageInner::Owned(_) | ValueStorageInner::OwnedReadOnly(_) => Ok(()),
            ValueStorageInner::Borrowed { validity, .. } => validity.check(),
        }
    }

    /// Check if this value reference is still valid for writing
    ///
    /// For owned values, always returns `Ok(())`.
    /// For read-only snapshots, returns `OwnedFieldReadOnly`.
    /// For borrowed values, checks both validity and write permission.
    #[inline(always)]
    fn check_valid_mut(&self) -> Result<(), StorageError> {
        match &self.inner {
            ValueStorageInner::Owned(_) => Ok(()),
            ValueStorageInner::OwnedReadOnly(_) => Err(StorageError::OwnedFieldReadOnly),
            ValueStorageInner::Borrowed { validity, .. } => validity.check_write(),
        }
    }

    /// Get the current value (returns a copy)
    ///
    /// For owned values, returns a copy.
    /// For borrowed values, copies the current value.
    ///
    /// # Errors
    /// Returns error if borrowed value is no longer valid
    #[inline(always)]
    pub fn get(&self) -> Result<T, StorageError> {
        Ok(*self.as_ref()?)
    }

    /// Get the validity flag for this storage
    ///
    /// Returns None for owned storage (always valid).
    /// Returns Some for borrowed storage.
    #[allow(dead_code)]
    fn validity(&self) -> Option<&ValidityFlagWithMode> {
        match &self.inner {
            ValueStorageInner::Owned(_) | ValueStorageInner::OwnedReadOnly(_) => None,
            ValueStorageInner::Borrowed { validity, .. } => Some(validity),
        }
    }

    /// Helper to borrow a field from the value storage
    ///
    /// This reduces boilerplate in field getters by unifying the owned/borrowed cases.
    /// Similar to ComponentStorage::borrow_field, but works with ValueStorage.
    ///
    /// For owned storage (including read-only snapshots), returns a read-only snapshot
    /// of the field. For borrowed storage, returns a borrowed pointer into the
    /// underlying data.
    pub fn borrow_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        match &self.inner {
            ValueStorageInner::Owned(value) | ValueStorageInner::OwnedReadOnly(value) => {
                Ok(S::snapshot(field_accessor(value)))
            }
            ValueStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;

                // SAFETY: We just checked validity above
                let value_ref = unsafe { &**ptr };
                let field_ref = field_accessor(value_ref);
                let field_ptr = field_ref as *const F as *mut F;

                // Share the validity flag with the field borrow
                Ok(unsafe { S::borrowed(field_ptr, validity.clone()) })
            }
        }
    }

    /// Helper to borrow a field and wrap it in the final Python type
    ///
    /// Combines `borrow_field` with `FromBorrowedStorage::from_borrowed` to reduce boilerplate.
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
