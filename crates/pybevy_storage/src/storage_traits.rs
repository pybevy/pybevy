//! Core storage traits for PyBevy
//!
//! These traits define the interface for storage types that support borrowed references
//! with validity tracking.

use crate::validity_guard::ValidityFlagWithMode;

/// Trait for storage types that can borrow field pointers with validity tracking
///
/// This is implemented by ValueStorage and FieldStorage to provide a unified
/// interface for creating borrowed field references.
pub trait BorrowableStorage<T>: Sized {
    /// Create a borrowed storage from a raw pointer and validity flag
    ///
    /// # Safety
    /// Caller must ensure:
    /// - `ptr` points to valid data of type `T`
    /// - The data at `ptr` lives at least as long as indicated by `validity`
    /// - No aliasing violations occur
    unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self;

    /// Create a read-only owned snapshot (copy) of the given value.
    ///
    /// Used when extracting fields from owned/temporary storage.
    /// The returned storage allows reads but errors on writes with
    /// `StorageError::OwnedFieldReadOnly`.
    fn snapshot(value: &T) -> Self
    where
        T: Clone;
}

/// Trait for Python wrapper types that can be created from borrowed storage
///
/// This enables the `borrow_field_as` helper method to return the final Python type
/// directly, reducing boilerplate from:
/// ```rust,ignore
/// Ok(PyVec3::from_borrowed(self.storage.borrow_field(|t| &t.translation)?))
/// ```
/// to:
/// ```rust,ignore
/// self.storage.borrow_field_as(|t| &t.translation)
/// ```
pub trait FromBorrowedStorage<S> {
    fn from_borrowed(storage: S) -> Self;
}
