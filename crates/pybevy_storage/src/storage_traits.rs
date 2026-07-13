//! Core storage traits for PyBevy
//!
//! These traits define the interface for storage types that support borrowed references
//! with validity tracking.

use crate::validity_guard::ValidityFlag;

/// Trait for storage types that can borrow field pointers with validity tracking
///
/// This is implemented by ValueStorage and FieldStorage to provide a unified
/// interface for creating borrowed field references. Read vs write access is
/// encoded by which constructor is used rather than by a runtime access mode.
pub trait BorrowableStorage<T>: Sized {
    /// Create a read-only borrowed storage from a const pointer and validity flag
    ///
    /// # Safety
    /// Caller must ensure:
    /// - `ptr` points to valid data of type `T`
    /// - The data at `ptr` lives at least as long as `validity` is non-Invalid
    /// - No `&mut T` aliasing the same memory exists while the flag is valid
    unsafe fn borrowed_ref(ptr: *const T, validity: ValidityFlag) -> Self;

    /// Create a mutable borrowed storage from a mut pointer and validity flag
    ///
    /// # Safety
    /// Caller must ensure:
    /// - `ptr` points to valid data of type `T` obtained from a `&mut T` chain
    /// - The data at `ptr` lives at least as long as `validity` is non-Invalid
    /// - No other reference aliases the same memory while the flag is valid
    unsafe fn borrowed_mut(ptr: *mut T, validity: ValidityFlag) -> Self;

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
