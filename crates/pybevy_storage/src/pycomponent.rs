//! Generic component storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for all PyBevy component types.
//! See the main crate's `pycomponent.rs` for detailed safety documentation.

use bevy::ecs::component::Component;

use crate::{
    storage_error::StorageError,
    storage_traits::BorrowableStorage,
    validity_guard::{ValidityFlag, ValidityFlagWithMode},
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

    /// Borrowed reference to component in ECS storage
    Borrowed {
        ptr: *mut T,
        validity: ValidityFlagWithMode,
    },
}

// SAFETY: See main crate's pycomponent.rs for detailed safety analysis
unsafe impl<T: Component + Send> Send for ComponentStorage<T> {}
unsafe impl<T: Component + Sync> Sync for ComponentStorage<T> {}

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
            (
                ComponentStorageInner::Borrowed { ptr: a, .. },
                ComponentStorageInner::Borrowed { ptr: b, .. },
            ) => a == b,
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

    /// Create borrowed component storage with direct pointer to component
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in ECS component storage
    /// - The pointer must remain valid while `validity` flag is true
    pub unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self {
        Self {
            inner: ComponentStorageInner::Borrowed { ptr, validity },
        }
    }

    /// Get immutable reference to the component, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, StorageError> {
        self.check_valid()?;
        Ok(unsafe { &*self.as_ptr() })
    }

    /// Get mutable reference to the component, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        self.check_valid_mut()?;
        Ok(unsafe { &mut *self.as_mut_ptr() })
    }

    #[inline(always)]
    fn as_ptr(&self) -> *const T {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => &**data as *const T,
            ComponentStorageInner::Borrowed { ptr, .. } => *ptr as *const T,
        }
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut T {
        match &mut self.inner {
            ComponentStorageInner::Owned { data, .. } => &mut **data as *mut T,
            ComponentStorageInner::Borrowed { ptr, .. } => *ptr,
        }
    }

    #[inline(always)]
    fn check_valid(&self) -> Result<(), StorageError> {
        match &self.inner {
            ComponentStorageInner::Owned { .. } => Ok(()),
            ComponentStorageInner::Borrowed { validity, .. } => validity.check(),
        }
    }

    #[inline(always)]
    fn check_valid_mut(&self) -> Result<(), StorageError> {
        match &self.inner {
            ComponentStorageInner::Owned { .. } => Ok(()),
            ComponentStorageInner::Borrowed { validity, .. } => validity.check_write(),
        }
    }

    /// Create a second handle to the same component data.
    ///
    /// For borrowed storage, shares the same pointer and validity flag.
    /// For owned storage, creates a new borrowed pointer into the owned data.
    ///
    /// Used when a sub-object (e.g., ActiveAnimation) needs to access
    /// the same component through its own storage handle.
    pub fn share_borrow(&self) -> Self {
        match &self.inner {
            ComponentStorageInner::Borrowed { ptr, validity } => Self {
                inner: ComponentStorageInner::Borrowed {
                    ptr: *ptr,
                    validity: validity.clone(),
                },
            },
            ComponentStorageInner::Owned { data, validity } => {
                let ptr = &**data as *const T as *mut T;
                Self {
                    inner: ComponentStorageInner::Borrowed {
                        ptr,
                        validity: validity
                            .with_access_mode(crate::validity_guard::AccessMode::Write),
                    },
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        matches!(self.inner, ComponentStorageInner::Owned { .. })
    }

    #[allow(dead_code)]
    pub fn is_borrowed(&self) -> bool {
        matches!(self.inner, ComponentStorageInner::Borrowed { .. })
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
            ComponentStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                let component_ref = unsafe { &**ptr };
                let field_ref = field_accessor(component_ref);
                let field_ptr = field_ref as *const F as *mut F;
                Ok(unsafe { S::borrowed(field_ptr, validity.clone()) })
            }
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
            ComponentStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                let component_ref = unsafe { &**ptr };
                match field_accessor(component_ref) {
                    Some(field_ref) => {
                        let field_ptr = field_ref as *const F as *mut F;
                        Ok(Some(unsafe { S::borrowed(field_ptr, validity.clone()) }))
                    }
                    None => Ok(None),
                }
            }
        }
    }
}

impl<T: Component + Clone> ComponentStorage<T> {
    /// Convert storage to owned component, consuming self
    pub fn into_owned(self) -> Result<T, StorageError> {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => Ok((**data).clone()),
            ComponentStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                Ok(unsafe { (**ptr).clone() })
            }
        }
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
}
