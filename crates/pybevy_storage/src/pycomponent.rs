//! Generic component storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for all PyBevy component types.
//! See the main crate's `pycomponent.rs` for detailed safety documentation.

use bevy::ecs::component::Component;

use crate::{
    borrowed::{BorrowedMut, BorrowedRef},
    storage_error::StorageError,
    storage_traits::BorrowableStorage,
    validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode},
};

/// Generic storage for PyBevy components
///
/// Supports two modes:
/// - `Owned`: Python-created component, fully owned by Python
/// - `Borrowed`: Reference to component in Bevy's ECS storage
#[derive(Debug)]
pub struct ComponentStorage<T: Component> {
    pub inner: ComponentStorageInner<T>,
}

#[derive(Debug)]
pub enum ComponentStorageInner<T: Component> {
    /// Python-created instance, fully owned with validity tracking
    Owned {
        data: Box<T>,
        validity: ValidityFlag,
    },

    /// Read-only borrow into ECS storage
    BorrowedRef(BorrowedRef<T>),

    /// Mutable borrow into ECS storage
    BorrowedMut(BorrowedMut<T>),
}

impl<T: Component> Drop for ComponentStorage<T> {
    fn drop(&mut self) {
        if let ComponentStorageInner::Owned { validity, .. } = &self.inner {
            validity.set_invalid();
        }
    }
}

impl<T: Component + PartialEq> PartialEq for ComponentStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (
                ComponentStorageInner::Owned { data: a, .. },
                ComponentStorageInner::Owned { data: b, .. },
            ) => **a == **b,
            (ComponentStorageInner::BorrowedRef(a), ComponentStorageInner::BorrowedRef(b)) => {
                a.as_ptr() == b.as_ptr()
            }
            (ComponentStorageInner::BorrowedMut(a), ComponentStorageInner::BorrowedMut(b)) => {
                a.as_ptr() == b.as_ptr()
            }
            _ => false,
        }
    }
}

impl<T: Component> ComponentStorage<T> {
    /// Create owned component storage with validity tracking
    pub fn owned(component: T) -> Self {
        Self {
            inner: ComponentStorageInner::Owned {
                data: Box::new(component),
                validity: ValidityFlag::new_write(),
            },
        }
    }

    /// Create a read-only borrowed component storage from a const pointer.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in ECS component storage
    /// - The pointer must remain valid while `validity` is non-Invalid
    /// - No `&mut T` aliasing the same component may exist while the flag is valid
    pub unsafe fn borrowed_ref(ptr: *const T, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ComponentStorageInner::BorrowedRef(unsafe { BorrowedRef::new(ptr, validity) }),
        }
    }

    /// Create a mutable borrowed component storage from a mut pointer.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in ECS component storage, from `&mut T`
    /// - The pointer must remain valid while `validity` is non-Invalid
    /// - No other reference may alias the same component while the flag is valid
    pub unsafe fn borrowed_mut(ptr: *mut T, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ComponentStorageInner::BorrowedMut(unsafe { BorrowedMut::new(ptr, validity) }),
        }
    }

    /// Create borrowed component storage, choosing read vs write from the transport mode.
    ///
    /// This is the bridge boundary constructor: `ValidityFlagWithMode` still carries
    /// the mode across the FFI layer, and is resolved here into a typed borrowed
    /// variant that no longer stores the mode.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in ECS component storage
    /// - `ptr` must be safe to write through when `validity.access_mode()` is `Write`
    pub unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self {
        match validity.access_mode() {
            // SAFETY: mode Write means ptr was obtained from a mutable borrow
            AccessMode::Write => unsafe { Self::borrowed_mut(ptr, validity.flag) },
            // SAFETY: read-only view of the same pointer
            _ => unsafe { Self::borrowed_ref(ptr as *const T, validity.flag) },
        }
    }

    /// Get immutable reference to the component, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, StorageError> {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => Ok(&**data),
            ComponentStorageInner::BorrowedRef(b) => b.get(),
            ComponentStorageInner::BorrowedMut(b) => b.get(),
        }
    }

    /// Get mutable reference to the component, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        match &mut self.inner {
            ComponentStorageInner::Owned { data, .. } => Ok(&mut **data),
            ComponentStorageInner::BorrowedRef(_) => Err(StorageError::ReadOnly),
            ComponentStorageInner::BorrowedMut(b) => b.get_mut(),
        }
    }

    /// Create a second handle to the same component data.
    ///
    /// Borrowed storage shares the same pointer and validity flag, preserving its
    /// read/write access (a `BorrowedRef` stays read-only, a `BorrowedMut` stays
    /// mutable). Owned storage yields a mutable borrow into the owned data.
    ///
    /// Used when a sub-object (e.g., ActiveAnimation) needs to access
    /// the same component through its own storage handle.
    pub fn share_borrow(&self) -> Self {
        let inner = match &self.inner {
            ComponentStorageInner::BorrowedRef(b) => ComponentStorageInner::BorrowedRef(b.clone()),
            ComponentStorageInner::BorrowedMut(b) => ComponentStorageInner::BorrowedMut(b.share()),
            ComponentStorageInner::Owned { data, validity } => {
                let ptr = &**data as *const T as *mut T;
                // SAFETY: ptr points into our own Box, valid while this storage lives;
                // the shared flag is invalidated by Drop before the Box is freed.
                ComponentStorageInner::BorrowedMut(unsafe {
                    BorrowedMut::new(ptr, validity.clone())
                })
            }
        };
        Self { inner }
    }

    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        matches!(self.inner, ComponentStorageInner::Owned { .. })
    }

    #[allow(dead_code)]
    pub fn is_borrowed(&self) -> bool {
        matches!(
            self.inner,
            ComponentStorageInner::BorrowedRef(_) | ComponentStorageInner::BorrowedMut(_)
        )
    }

    /// Borrow a field from the component storage
    ///
    /// For owned storage, returns a read-only snapshot of the field.
    /// For borrowed storage, returns a borrowed pointer into the ECS data.
    pub fn borrow_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => Ok(S::snapshot(field_accessor(&**data))),
            ComponentStorageInner::BorrowedRef(b) => b.borrow_field(field_accessor),
            ComponentStorageInner::BorrowedMut(b) => b.borrow_field(field_accessor),
        }
    }

    /// Borrow a field and wrap it in the final Python type
    pub fn borrow_field_as<F: Clone, S, W>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: crate::storage_traits::FromBorrowedStorage<S>,
    {
        Ok(W::from_borrowed(self.borrow_field(field_accessor)?))
    }

    /// Borrow an optional field from the component storage
    ///
    /// For owned storage, returns a read-only snapshot of the field.
    /// For borrowed storage, returns a borrowed pointer into the ECS data.
    pub fn borrow_optional_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &Option<F>,
    ) -> Result<Option<S>, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => match field_accessor(&**data) {
                Some(field_ref) => Ok(Some(S::snapshot(field_ref))),
                None => Ok(None),
            },
            ComponentStorageInner::BorrowedRef(b) => b.borrow_optional_field(field_accessor),
            ComponentStorageInner::BorrowedMut(b) => b.borrow_optional_field(field_accessor),
        }
    }
}

impl<T: Component + Clone> ComponentStorage<T> {
    /// Convert storage to owned component, consuming self
    pub fn into_owned(self) -> Result<T, StorageError> {
        Ok(self.as_ref()?.clone())
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::component::Component;

    use super::*;
    use crate::{AccessMode, ValidityFlag, ValidityGuard};

    #[derive(Clone, Debug, PartialEq, Component)]
    struct TestComponent {
        value: i32,
    }

    #[test]
    fn test_owned_storage() {
        let storage = ComponentStorage::owned(TestComponent { value: 42 });
        assert!(storage.is_owned());
        assert!(!storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().value, 42);
    }

    #[test]
    fn test_owned_mutation() {
        let mut storage = ComponentStorage::owned(TestComponent { value: 42 });
        storage.as_mut().unwrap().value = 100;
        assert_eq!(storage.as_ref().unwrap().value, 100);
    }

    #[test]
    fn test_borrowed_storage() {
        let mut component = TestComponent { value: 42 };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage = unsafe {
            ComponentStorage::borrowed(&mut component as *mut TestComponent, validity.clone())
        };

        assert!(!storage.is_owned());
        assert!(storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().value, 42);
    }

    #[test]
    fn test_borrowed_mutation() {
        let mut component = TestComponent { value: 42 };
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);

        let mut storage = unsafe {
            ComponentStorage::borrowed(&mut component as *mut TestComponent, validity.clone())
        };

        storage.as_mut().unwrap().value = 100;
        assert_eq!(component.value, 100);
        assert_eq!(storage.as_ref().unwrap().value, 100);
    }

    #[test]
    fn test_validity_enforcement() {
        let mut component = TestComponent { value: 42 };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage = unsafe {
            ComponentStorage::borrowed(&mut component as *mut TestComponent, validity.clone())
        };

        // Should work while valid (with guard active)
        {
            let _guard = ValidityGuard::new(validity.flag.clone());
            assert!(storage.as_ref().is_ok());
        }

        // Should fail when invalid (guard dropped)
        assert!(storage.as_ref().is_err());
    }

    #[test]
    fn test_write_permission_enforcement() {
        let mut component = TestComponent { value: 42 };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read); // Read-only borrow!

        let mut storage = unsafe {
            ComponentStorage::borrowed(&mut component as *mut TestComponent, validity.clone())
        };

        // Read should work (with guard active)
        {
            let _guard = ValidityGuard::new(validity.flag.clone());
            assert!(storage.as_ref().is_ok());

            // Write should fail (borrowed as Ref, not Mut)
            assert!(storage.as_mut().is_err());
        }
    }

    #[test]
    fn test_into_owned_from_owned() {
        let storage = ComponentStorage::owned(TestComponent { value: 42 });
        let component = storage.into_owned().unwrap();
        assert_eq!(component.value, 42);
    }

    #[test]
    fn test_into_owned_from_borrowed() {
        let mut component = TestComponent { value: 42 };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage =
            unsafe { ComponentStorage::borrowed(&mut component as *mut TestComponent, validity) };

        let owned = storage.into_owned().unwrap();
        assert_eq!(owned.value, 42);
    }

    #[test]
    fn test_into_owned_from_invalid_borrowed_fails() {
        let mut component = TestComponent { value: 42 };
        let validity_flag = ValidityFlag::new_read();
        let validity = validity_flag.with_access_mode(AccessMode::Read);

        let storage = unsafe {
            ComponentStorage::borrowed(&mut component as *mut TestComponent, validity.clone())
        };

        // Activate then drop guard to invalidate
        {
            let _guard = ValidityGuard::new(validity_flag.clone());
        }
        // Guard dropped - validity is now Invalid

        // Should fail because validity was invalidated
        assert!(storage.into_owned().is_err());
    }

    #[derive(Clone, Debug, PartialEq, Component)]
    struct NestedComponent {
        x: f32,
        y: f32,
    }

    #[test]
    fn test_owned_borrow_field_returns_read_only_snapshot() {
        use crate::value_storage::ValueStorage;

        let storage = ComponentStorage::owned(NestedComponent { x: 1.0, y: 2.0 });
        let field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();

        // Should be an OwnedReadOnly snapshot, not a borrowed pointer
        assert!(field.is_owned_read_only());
        assert_eq!(field.get().unwrap(), 1.0);
    }

    #[test]
    fn test_owned_borrow_field_snapshot_rejects_writes() {
        use crate::value_storage::ValueStorage;

        let storage = ComponentStorage::owned(NestedComponent { x: 1.0, y: 2.0 });
        let mut field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();

        assert!(matches!(
            field.as_mut(),
            Err(crate::StorageError::OwnedFieldReadOnly)
        ));
    }

    #[test]
    fn test_owned_borrow_field_snapshot_survives_parent_drop() {
        use crate::value_storage::ValueStorage;

        let field: ValueStorage<f32>;
        {
            let storage = ComponentStorage::owned(NestedComponent { x: 42.0, y: 0.0 });
            field = storage.borrow_field(|c| &c.x).unwrap();
            // storage dropped here
        }
        // Snapshot is independent — still readable after parent is dropped
        assert_eq!(field.get().unwrap(), 42.0);
    }

    #[test]
    fn test_borrowed_borrow_field_still_returns_borrowed() {
        use crate::value_storage::ValueStorage;

        let mut component = NestedComponent { x: 5.0, y: 6.0 };
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let storage =
            unsafe { ComponentStorage::borrowed(&mut component as *mut NestedComponent, validity) };

        let field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();
        // Borrowed storage still returns borrowed sub-fields
        assert!(field.is_borrowed());
    }

    #[test]
    fn test_borrowed_ref_rejects_writes() {
        let component = TestComponent { value: 7 };
        let validity = ValidityFlag::new_read();
        let mut storage =
            unsafe { ComponentStorage::borrowed_ref(&component as *const TestComponent, validity) };
        assert!(storage.as_ref().is_ok());
        assert!(matches!(storage.as_mut(), Err(StorageError::ReadOnly)));
    }

    #[test]
    fn test_borrowed_mut_allows_writes() {
        let mut component = TestComponent { value: 7 };
        let validity = ValidityFlag::new_write();
        let mut storage = unsafe {
            ComponentStorage::borrowed_mut(&mut component as *mut TestComponent, validity)
        };
        *storage.as_mut().unwrap() = TestComponent { value: 99 };
        assert_eq!(component.value, 99);
    }

    #[test]
    fn test_share_borrow_from_owned_is_writable() {
        // An owned parent hands out a mutable borrow that persists back to it.
        let owner = ComponentStorage::owned(TestComponent { value: 1 });
        let mut shared = owner.share_borrow();
        assert!(shared.is_borrowed());
        shared.as_mut().unwrap().value = 42;
        assert_eq!(owner.as_ref().unwrap().value, 42);
    }

    #[test]
    fn test_share_borrow_preserves_read_only_mode() {
        // Sharing a read-only borrow keeps it read-only.
        let component = TestComponent { value: 3 };
        let validity = ValidityFlag::new_read();
        let storage =
            unsafe { ComponentStorage::borrowed_ref(&component as *const TestComponent, validity) };
        let mut shared = storage.share_borrow();
        assert!(shared.as_ref().is_ok());
        assert!(matches!(shared.as_mut(), Err(StorageError::ReadOnly)));
    }

    #[test]
    fn test_share_borrow_preserves_write_mode() {
        // Sharing a mutable borrow keeps it mutable.
        let mut component = TestComponent { value: 3 };
        let validity = ValidityFlag::new_write();
        let storage = unsafe {
            ComponentStorage::borrowed_mut(&mut component as *mut TestComponent, validity)
        };
        let mut shared = storage.share_borrow();
        shared.as_mut().unwrap().value = 55;
        assert_eq!(component.value, 55);
    }

    #[test]
    fn test_borrow_field_inherits_mutability() {
        use crate::value_storage::ValueStorage;

        // Mutable parent -> mutable child field.
        let mut component = NestedComponent { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_write();
        let storage = unsafe {
            ComponentStorage::borrowed_mut(&mut component as *mut NestedComponent, validity)
        };
        let mut field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();
        *field.as_mut().unwrap() = 9.0;
        assert_eq!(component.x, 9.0);

        // Read-only parent -> read-only child field.
        let component = NestedComponent { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_read();
        let storage = unsafe {
            ComponentStorage::borrowed_ref(&component as *const NestedComponent, validity)
        };
        let mut field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();
        assert!(matches!(field.as_mut(), Err(StorageError::ReadOnly)));
    }
}
