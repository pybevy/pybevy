//! Generic resource storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for all PyBevy resource types,
//! enabling zero-copy access to Bevy resources through borrowed references.

use bevy::ecs::prelude::Resource;

use crate::{
    storage_error::StorageError,
    validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode},
};

/// Generic storage for PyBevy resources
///
/// Supports two modes:
/// - `Owned`: Python-created resource, fully owned by Python
/// - `Borrowed`: Reference to resource in Bevy's World storage
#[derive(Debug)]
pub struct ResourceStorage<T: Resource> {
    pub inner: ResourceStorageInner<T>,
}

#[derive(Debug)]
pub enum ResourceStorageInner<T: Resource> {
    /// Python-created instance, fully owned with validity tracking
    Owned {
        data: Box<T>,
        validity: ValidityFlag,
    },

    /// Borrowed reference to resource in World storage
    Borrowed {
        ptr: *mut T,
        validity: ValidityFlagWithMode,
    },
}

// SAFETY: ResourceStorage is Send because:
// - Box<T> is Send when T is Send
// - Raw pointer is just an address
// - ValidityFlag (Arc<AtomicU8>) is Send + Sync
// - Validity checking prevents unsafe access
unsafe impl<T: Resource + Send> Send for ResourceStorage<T> {}

// SAFETY: ResourceStorage is Sync because:
// - Access is controlled by validity checking
// - ValidityFlag uses atomic operations
// - We only allow access when validity flag is true
unsafe impl<T: Resource + Sync> Sync for ResourceStorage<T> {}

impl<T: Resource + Clone> Clone for ResourceStorage<T> {
    fn clone(&self) -> Self {
        match &self.inner {
            ResourceStorageInner::Owned { data, validity: _ } => Self {
                inner: ResourceStorageInner::Owned {
                    data: Box::new((**data).clone()),
                    validity: ValidityFlag::new_write(),
                },
            },
            ResourceStorageInner::Borrowed { ptr, validity } => Self {
                inner: ResourceStorageInner::Borrowed {
                    ptr: *ptr,
                    validity: validity.clone(),
                },
            },
        }
    }
}

impl<T: Resource> Drop for ResourceStorage<T> {
    fn drop(&mut self) {
        if let ResourceStorageInner::Owned { validity, .. } = &self.inner {
            validity.set_invalid();
        }
    }
}

impl<T: Resource + PartialEq> PartialEq for ResourceStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (
                ResourceStorageInner::Owned { data: a, .. },
                ResourceStorageInner::Owned { data: b, .. },
            ) => **a == **b,
            (
                ResourceStorageInner::Borrowed { ptr: a, .. },
                ResourceStorageInner::Borrowed { ptr: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

impl<T: Resource> ResourceStorage<T> {
    /// Create owned resource storage with validity tracking
    pub fn owned(resource: T) -> Self {
        Self {
            inner: ResourceStorageInner::Owned {
                data: Box::new(resource),
                validity: ValidityFlag::new_write(),
            },
        }
    }

    /// Create borrowed resource storage with direct pointer and validity mode
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in World resource storage
    /// - The pointer must remain valid while `validity` flag is true
    pub unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self {
        Self {
            inner: ResourceStorageInner::Borrowed { ptr, validity },
        }
    }

    /// Create read-only borrowed resource storage (for Res[T])
    ///
    /// Convenience method that creates borrowed storage with Read access mode.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in World resource storage
    /// - The pointer must remain valid while `validity` flag is true
    /// - Caller must ensure no mutable references exist
    pub unsafe fn borrowed_read(ptr: *const T, validity: ValidityFlag) -> Self {
        Self {
            inner: ResourceStorageInner::Borrowed {
                ptr: ptr as *mut T,
                validity: validity.with_access_mode(AccessMode::Read),
            },
        }
    }

    /// Create mutable borrowed resource storage (for ResMut[T])
    ///
    /// Convenience method that creates borrowed storage with Write access mode.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in World resource storage
    /// - The pointer must remain valid while `validity` flag is true
    /// - Caller must ensure exclusive mutable access
    pub unsafe fn borrowed_write(ptr: *mut T, validity: ValidityFlag) -> Self {
        Self {
            inner: ResourceStorageInner::Borrowed {
                ptr,
                validity: validity.with_access_mode(AccessMode::Write),
            },
        }
    }

    /// Get immutable reference to the resource, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, StorageError> {
        self.check_valid()?;
        Ok(unsafe { &*self.as_ptr() })
    }

    /// Get mutable reference to the resource, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        self.check_valid_mut()?;
        Ok(unsafe { &mut *self.as_mut_ptr() })
    }

    #[inline(always)]
    fn as_ptr(&self) -> *const T {
        match &self.inner {
            ResourceStorageInner::Owned { data, .. } => &**data as *const T,
            ResourceStorageInner::Borrowed { ptr, .. } => *ptr as *const T,
        }
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut T {
        match &mut self.inner {
            ResourceStorageInner::Owned { data, .. } => &mut **data as *mut T,
            ResourceStorageInner::Borrowed { ptr, .. } => *ptr,
        }
    }

    #[inline(always)]
    fn check_valid(&self) -> Result<(), StorageError> {
        match &self.inner {
            ResourceStorageInner::Owned { .. } => Ok(()),
            ResourceStorageInner::Borrowed { validity, .. } => validity.check(),
        }
    }

    #[inline(always)]
    fn check_valid_mut(&self) -> Result<(), StorageError> {
        match &self.inner {
            ResourceStorageInner::Owned { .. } => Ok(()),
            ResourceStorageInner::Borrowed { validity, .. } => validity.check_write(),
        }
    }

    /// Borrow a field from the resource storage
    ///
    /// For owned storage, returns a read-only snapshot of the field.
    /// For borrowed storage, returns a borrowed pointer into the World data.
    pub fn borrow_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: crate::BorrowableStorage<F>,
    {
        match &self.inner {
            ResourceStorageInner::Owned { data, .. } => Ok(S::snapshot(field_accessor(&**data))),
            ResourceStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                let resource_ref = unsafe { &**ptr };
                let field_ref = field_accessor(resource_ref);
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
        S: crate::BorrowableStorage<F>,
        W: crate::FromBorrowedStorage<S>,
    {
        Ok(W::from_borrowed(self.borrow_field(field_accessor)?))
    }

    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        matches!(self.inner, ResourceStorageInner::Owned { .. })
    }

    #[allow(dead_code)]
    pub fn is_borrowed(&self) -> bool {
        matches!(self.inner, ResourceStorageInner::Borrowed { .. })
    }
}

impl<T: Resource + Clone> ResourceStorage<T> {
    /// Convert storage to owned resource, consuming self
    pub fn into_owned(self) -> Result<T, StorageError> {
        match &self.inner {
            ResourceStorageInner::Owned { data, .. } => Ok((**data).clone()),
            ResourceStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                Ok(unsafe { (**ptr).clone() })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::prelude::Resource;

    use super::*;
    use crate::{AccessMode, ValidityFlag, ValidityGuard};

    #[derive(Clone, Debug, PartialEq, Resource)]
    struct TestResource {
        value: i32,
    }

    #[derive(Clone, Debug, PartialEq, Resource)]
    struct NestedResource {
        x: f32,
        y: f32,
    }

    #[test]
    fn test_owned_storage() {
        let storage = ResourceStorage::owned(TestResource { value: 42 });
        assert!(storage.is_owned());
        assert!(!storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().value, 42);
    }

    #[test]
    fn test_owned_mutation() {
        let mut storage = ResourceStorage::owned(TestResource { value: 42 });
        storage.as_mut().unwrap().value = 100;
        assert_eq!(storage.as_ref().unwrap().value, 100);
    }

    #[test]
    fn test_borrowed_read_storage() {
        let resource = TestResource { value: 42 };
        let validity = ValidityFlag::new_read();

        let storage = unsafe {
            ResourceStorage::borrowed_read(&resource as *const TestResource, validity.clone())
        };

        assert!(!storage.is_owned());
        assert!(storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().value, 42);
    }

    #[test]
    fn test_borrowed_read_rejects_writes() {
        let resource = TestResource { value: 42 };
        let validity = ValidityFlag::new_read();

        let mut storage = unsafe {
            ResourceStorage::borrowed_read(&resource as *const TestResource, validity.clone())
        };

        assert!(storage.as_ref().is_ok());
        assert!(storage.as_mut().is_err());
    }

    #[test]
    fn test_borrowed_write_storage() {
        let mut resource = TestResource { value: 42 };
        let validity = ValidityFlag::new_write();

        let mut storage = unsafe {
            ResourceStorage::borrowed_write(&mut resource as *mut TestResource, validity.clone())
        };

        storage.as_mut().unwrap().value = 100;
        assert_eq!(resource.value, 100);
        assert_eq!(storage.as_ref().unwrap().value, 100);
    }

    #[test]
    fn test_validity_enforcement() {
        let resource = TestResource { value: 42 };
        let flag = ValidityFlag::new();

        let storage = unsafe {
            ResourceStorage::borrowed_read(&resource as *const TestResource, flag.clone())
        };

        {
            let _guard = ValidityGuard::new(flag.clone());
            assert!(storage.as_ref().is_ok());
        }

        assert!(storage.as_ref().is_err());
    }

    #[test]
    fn test_write_permission_enforcement() {
        let resource = TestResource { value: 42 };
        let flag = ValidityFlag::new();

        let mut storage = unsafe {
            ResourceStorage::borrowed_read(&resource as *const TestResource, flag.clone())
        };

        {
            let _guard = ValidityGuard::new(flag.clone());
            assert!(storage.as_ref().is_ok());
            assert!(storage.as_mut().is_err());
        }
    }

    #[test]
    fn test_into_owned_from_owned() {
        let storage = ResourceStorage::owned(TestResource { value: 42 });
        let resource = storage.into_owned().unwrap();
        assert_eq!(resource.value, 42);
    }

    #[test]
    fn test_into_owned_from_borrowed() {
        let resource = TestResource { value: 42 };
        let validity = ValidityFlag::new_read();

        let storage =
            unsafe { ResourceStorage::borrowed_read(&resource as *const TestResource, validity) };

        let owned = storage.into_owned().unwrap();
        assert_eq!(owned.value, 42);
    }

    #[test]
    fn test_into_owned_from_invalid_borrowed_fails() {
        let resource = TestResource { value: 42 };
        let flag = ValidityFlag::new();

        let storage = unsafe {
            ResourceStorage::borrowed_read(&resource as *const TestResource, flag.clone())
        };

        {
            let _guard = ValidityGuard::new(flag.clone());
        }

        assert!(storage.into_owned().is_err());
    }

    #[test]
    fn test_drop_invalidates_owned() {
        let validity_clone;
        {
            let storage = ResourceStorage::owned(TestResource { value: 42 });
            match &storage.inner {
                ResourceStorageInner::Owned { validity, .. } => {
                    validity_clone = validity.clone();
                }
                _ => unreachable!(),
            }
            assert_eq!(validity_clone.get_mode(), AccessMode::Write);
        }
        assert_eq!(validity_clone.get_mode(), AccessMode::Invalid);
    }

    #[test]
    fn test_clone_owned_creates_independent_storage() {
        let mut storage = ResourceStorage::owned(TestResource { value: 42 });
        let mut cloned = storage.clone();

        storage.as_mut().unwrap().value = 100;
        assert_eq!(cloned.as_ref().unwrap().value, 42);

        cloned.as_mut().unwrap().value = 200;
        assert_eq!(storage.as_ref().unwrap().value, 100);
    }

    #[test]
    fn test_borrow_field_from_owned() {
        use crate::value_storage::ValueStorage;

        let storage = ResourceStorage::owned(NestedResource { x: 1.0, y: 2.0 });
        let field: ValueStorage<f32> = storage.borrow_field(|r| &r.x).unwrap();

        assert!(field.is_owned_read_only());
        assert_eq!(field.get().unwrap(), 1.0);
    }

    #[test]
    fn test_borrow_field_from_borrowed() {
        use crate::value_storage::ValueStorage;

        let mut resource = NestedResource { x: 5.0, y: 6.0 };
        let validity = ValidityFlag::new_write();

        let storage = unsafe {
            ResourceStorage::borrowed_write(&mut resource as *mut NestedResource, validity)
        };

        let field: ValueStorage<f32> = storage.borrow_field(|r| &r.x).unwrap();
        assert!(field.is_borrowed());
        assert_eq!(field.get().unwrap(), 5.0);
    }

    #[test]
    fn test_borrow_field_invalid_after_guard_dropped() {
        use crate::value_storage::ValueStorage;

        let mut resource = NestedResource { x: 1.0, y: 2.0 };
        let flag = ValidityFlag::new();

        let storage = unsafe {
            ResourceStorage::borrowed_write(&mut resource as *mut NestedResource, flag.clone())
        };

        let field: ValueStorage<f32>;
        {
            let _guard = ValidityGuard::new(flag.clone());
            field = storage.borrow_field(|r| &r.x).unwrap();
            assert!(field.as_ref().is_ok());
        }

        assert!(field.as_ref().is_err());
    }
}
