//! Typed borrowed primitives shared by every storage type
//!
//! `BorrowedRef<T>` and `BorrowedMut<T>` encode read vs write access at the type
//! level instead of via a runtime `AccessMode` flag. Each carries only a raw
//! pointer and a plain `ValidityFlag` (the "system still executing" gate); the
//! mode that used to live in `ValidityFlagWithMode` is now the choice of which
//! wrapper holds the pointer.
//!
//! All six storage types (`ValueStorage`, `FieldStorage`, `ListStorage`,
//! `ComponentStorage`, `ResourceStorage`, `AssetStorage`) wrap these two types in
//! their borrowed variants, so the `Send`/`Sync` and `borrow_field` logic lives
//! here once rather than being duplicated per storage type.

use crate::{
    storage_error::StorageError, storage_traits::BorrowableStorage, validity_guard::ValidityFlag,
};

/// Read-only borrow into parent storage (component, resource, or another borrow).
///
/// Holds a `*const T`; `as_mut` is impossible because this type has no mutable
/// accessor. Sub-borrows produced via `borrow_field` are themselves read-only.
#[derive(Debug)]
pub struct BorrowedRef<T> {
    ptr: *const T,
    validity: ValidityFlag,
}

/// Mutable borrow into parent storage.
///
/// Holds a `*mut T` obtained from a `&mut T` chain. Deliberately not `Clone`:
/// duplicating a mutable alias must be explicit, via `share` (another
/// `BorrowedMut`) or `clone_as_ref` (a read-only downgrade).
#[derive(Debug)]
pub struct BorrowedMut<T> {
    ptr: *mut T,
    validity: ValidityFlag,
}

// SAFETY: the raw pointer is just an address and access is gated by the
// `ValidityFlag` (Arc<AtomicU8>), which is itself Send + Sync. The flag is
// invalidated (RAII) when the owning system exits, so the pointer is never
// dereferenced outside the borrow's valid window.
unsafe impl<T: Send> Send for BorrowedRef<T> {}
// SAFETY: same argument as the impl above
unsafe impl<T: Sync> Sync for BorrowedRef<T> {}
// SAFETY: same argument as the impl above
unsafe impl<T: Send> Send for BorrowedMut<T> {}
// SAFETY: same argument as the impl above
unsafe impl<T: Sync> Sync for BorrowedMut<T> {}

impl<T> Clone for BorrowedRef<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            validity: self.validity.clone(),
        }
    }
}

impl<T> BorrowedRef<T> {
    /// # Safety
    /// - `ptr` must point to valid `T` for as long as `validity` is non-Invalid.
    /// - No `&mut T` aliasing the same memory may exist while the flag is valid.
    #[inline(always)]
    pub unsafe fn new(ptr: *const T, validity: ValidityFlag) -> Self {
        Self { ptr, validity }
    }

    #[inline(always)]
    pub fn get(&self) -> Result<&T, StorageError> {
        self.validity.check_read()?;
        // SAFETY: validity checked above; ptr stays valid while the flag is set
        Ok(unsafe { &*self.ptr })
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const T {
        self.ptr
    }

    /// Borrow a sub-field, inheriting read-only access.
    pub fn borrow_field<F, S>(
        &self,
        field_accessor: impl FnOnce(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        self.validity.check_read()?;
        // SAFETY: validity checked above; ptr is stable during system execution.
        let field_ptr = field_accessor(unsafe { &*self.ptr }) as *const F;
        // SAFETY: field_ptr derives from the checked parent ptr and shares its flag
        Ok(unsafe { S::borrowed_ref(field_ptr, self.validity.clone()) })
    }

    /// Borrow an optional sub-field, inheriting read-only access.
    pub fn borrow_optional_field<F, S>(
        &self,
        field_accessor: impl FnOnce(&T) -> &Option<F>,
    ) -> Result<Option<S>, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        self.validity.check_read()?;
        // SAFETY: validity checked above; ptr is stable during system execution.
        match field_accessor(unsafe { &*self.ptr }) {
            Some(field_ref) => {
                let field_ptr = field_ref as *const F;
                // SAFETY: field_ptr derives from the checked parent ptr and shares its flag
                Ok(Some(unsafe {
                    S::borrowed_ref(field_ptr, self.validity.clone())
                }))
            }
            None => Ok(None),
        }
    }
}

impl<T> BorrowedMut<T> {
    /// # Safety
    /// - `ptr` must point to valid `T` for as long as `validity` is non-Invalid.
    /// - `ptr` must have been obtained from a `&mut T` chain.
    /// - No other reference may alias the same memory while the flag is valid.
    #[inline(always)]
    pub unsafe fn new(ptr: *mut T, validity: ValidityFlag) -> Self {
        Self { ptr, validity }
    }

    #[inline(always)]
    pub fn get(&self) -> Result<&T, StorageError> {
        self.validity.check_read()?;
        // SAFETY: validity checked above; ptr stays valid while the flag is set
        Ok(unsafe { &*self.ptr })
    }

    #[inline(always)]
    pub fn get_mut(&mut self) -> Result<&mut T, StorageError> {
        self.validity.check_write()?;
        // SAFETY: validity checked above; ptr came from a &mut chain per new()'s contract
        Ok(unsafe { &mut *self.ptr })
    }

    /// Create a second mutable handle to the same data, sharing the validity flag.
    ///
    /// Used for intentional pointer sharing (e.g. `AnimationPlayer` handing a
    /// borrow to `ActiveAnimation`). Not exposed as `Clone` to keep accidental
    /// aliasing out of derive-generated code.
    #[inline(always)]
    pub fn share(&self) -> Self {
        // SAFETY: same ptr and flag; the original new() contract still holds
        unsafe { Self::new(self.ptr, self.validity.clone()) }
    }

    /// Downgrade to a read-only `BorrowedRef`, sharing the same validity flag.
    #[inline(always)]
    pub fn clone_as_ref(&self) -> BorrowedRef<T> {
        // SAFETY: read-only downgrade sharing the same ptr and flag
        unsafe { BorrowedRef::new(self.ptr as *const T, self.validity.clone()) }
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const T {
        self.ptr as *const T
    }

    /// Borrow a sub-field, inheriting mutable access.
    pub fn borrow_field<F, S>(
        &self,
        field_accessor: impl FnOnce(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        self.validity.check_read()?;
        // SAFETY: validity checked above; ptr is stable during system execution.
        let field_ptr = field_accessor(unsafe { &*self.ptr }) as *const F as *mut F;
        // SAFETY: field_ptr derives from the checked parent ptr and shares its flag
        Ok(unsafe { S::borrowed_mut(field_ptr, self.validity.clone()) })
    }

    /// Borrow an optional sub-field, inheriting mutable access.
    pub fn borrow_optional_field<F, S>(
        &self,
        field_accessor: impl FnOnce(&T) -> &Option<F>,
    ) -> Result<Option<S>, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        self.validity.check_read()?;
        // SAFETY: validity checked above; ptr is stable during system execution.
        match field_accessor(unsafe { &*self.ptr }) {
            Some(field_ref) => {
                let field_ptr = field_ref as *const F as *mut F;
                // SAFETY: field_ptr derives from the checked parent ptr and shares its flag
                Ok(Some(unsafe {
                    S::borrowed_mut(field_ptr, self.validity.clone())
                }))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// get_mut requires the flag to be in Write state, even though mutability
    /// is otherwise encoded in the type: a master flag downgraded to Read
    /// (or invalidated) must reject writes through an existing BorrowedMut.
    #[test]
    fn borrowed_mut_get_mut_requires_write_state() {
        let mut value = 7u32;
        let flag = ValidityFlag::new_write();
        // SAFETY: value outlives the borrow within this test scope
        let mut borrow = unsafe { BorrowedMut::new(&mut value as *mut u32, flag.clone()) };

        *borrow.get_mut().unwrap() = 8;
        assert_eq!(*borrow.get().unwrap(), 8);

        let read_flag = ValidityFlag::new_read();
        // SAFETY: same value, still live
        let mut read_state_borrow = unsafe { BorrowedMut::new(&mut value as *mut u32, read_flag) };
        assert!(read_state_borrow.get().is_ok());
        assert!(matches!(
            read_state_borrow.get_mut(),
            Err(StorageError::ReadOnly)
        ));

        flag.set_invalid();
        assert!(matches!(borrow.get_mut(), Err(StorageError::InvalidAccess)));
    }
}
