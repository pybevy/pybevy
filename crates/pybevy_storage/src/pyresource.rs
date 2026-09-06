//! Generic resource storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for all PyBevy resource types,
//! enabling zero-copy access to Bevy resources through borrowed references.

use bevy::ecs::{component::ComponentId, entity::Entity, prelude::Resource, world::World};

use crate::{
    borrowed::{BorrowedMut, BorrowedRef, RevalidatingField},
    component_change::ComponentWriteContext,
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

    /// Read-only borrow into World storage
    BorrowedRef(BorrowedRef<T>),

    /// Mutable borrow into World storage
    BorrowedMut(BorrowedMut<T>),

    /// World entity/component identity re-resolved on every access.
    Revalidating(Box<RevalidatingField>),
}

impl<T: Resource + Clone> Clone for ResourceStorage<T> {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            ResourceStorageInner::Owned { data, validity: _ } => ResourceStorageInner::Owned {
                data: Box::new((**data).clone()),
                validity: ValidityFlag::new_write(),
            },
            ResourceStorageInner::BorrowedRef(b) => ResourceStorageInner::BorrowedRef(b.clone()),
            // A cloned mutable borrow downgrades to read-only to avoid aliasing.
            ResourceStorageInner::BorrowedMut(b) => {
                ResourceStorageInner::BorrowedRef(b.clone_as_ref())
            }
            ResourceStorageInner::Revalidating(r) => ResourceStorageInner::Revalidating(r.clone()),
        };
        Self { inner }
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
            (ResourceStorageInner::BorrowedRef(a), ResourceStorageInner::BorrowedRef(b)) => {
                a.as_ptr() == b.as_ptr()
            }
            (ResourceStorageInner::BorrowedMut(a), ResourceStorageInner::BorrowedMut(b)) => {
                a.as_ptr() == b.as_ptr()
            }
            (ResourceStorageInner::Revalidating(a), ResourceStorageInner::Revalidating(b)) => {
                a.same_identity(b)
            }
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

    /// Create a read-only borrowed resource storage from a const pointer.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in World resource storage
    /// - The pointer must remain valid while `validity` is non-Invalid
    /// - No `&mut T` aliasing the same resource may exist while the flag is valid
    pub unsafe fn borrowed_ref(ptr: *const T, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ResourceStorageInner::BorrowedRef(unsafe { BorrowedRef::new(ptr, validity) }),
        }
    }

    /// Create a mutable borrowed resource storage from a mut pointer.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in World resource storage, from `&mut T`
    /// - The pointer must remain valid while `validity` is non-Invalid
    /// - No other reference may alias the same resource while the flag is valid
    pub unsafe fn borrowed_mut(ptr: *mut T, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ResourceStorageInner::BorrowedMut(unsafe { BorrowedMut::new(ptr, validity) }),
        }
    }

    /// Create mutable query storage that marks the resource component only on a real write.
    ///
    /// # Safety
    /// The requirements of [`Self::borrowed_mut`] and
    /// [`ComponentWriteContext::new`] apply.
    pub unsafe fn borrowed_mut_tracked(
        ptr: *mut T,
        validity: ValidityFlag,
        context: ComponentWriteContext,
    ) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged.
            inner: ResourceStorageInner::BorrowedMut(unsafe {
                BorrowedMut::new_tracked(ptr, validity, context)
            }),
        }
    }

    /// Create borrowed resource storage, choosing read vs write from the transport mode.
    ///
    /// This is the bridge boundary constructor: `ValidityFlagWithMode` still carries
    /// the mode across the FFI layer, and is resolved here into a typed borrowed
    /// variant that no longer stores the mode.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in World resource storage
    /// - `ptr` must be safe to write through when `validity.access_mode()` is `Write`
    pub unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self {
        let component_write = validity.component_write_context();
        match validity.access_mode() {
            // SAFETY: mode Write means ptr was obtained from a mutable borrow.
            AccessMode::Write => match component_write {
                // SAFETY: forwards this constructor's pointer and context contract.
                Some(context) => unsafe { Self::borrowed_mut_tracked(ptr, validity.flag, context) },
                // SAFETY: forwards this constructor's pointer contract.
                None => unsafe { Self::borrowed_mut(ptr, validity.flag) },
            },
            // SAFETY: read-only view of the same pointer
            _ => unsafe { Self::borrowed_ref(ptr as *const T, validity.flag) },
        }
    }

    /// Build storage for long-lived World entity access without caching a pointer.
    ///
    /// # Safety
    /// `world_ptr` must remain live while `validity` permits access, and
    /// `(entity, component_id)` must identify a value of `T`.
    pub unsafe fn revalidating(
        world_ptr: *mut World,
        entity: Entity,
        component_id: ComponentId,
        validity: ValidityFlagWithMode,
    ) -> Self {
        Self {
            // SAFETY: forwards this constructor's world identity and validity contract.
            inner: ResourceStorageInner::Revalidating(Box::new(unsafe {
                RevalidatingField::new(world_ptr, entity, component_id, 0, validity)
            })),
        }
    }

    /// Create read-only borrowed resource storage (for Res[T])
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in World resource storage
    /// - The pointer must remain valid while `validity` is non-Invalid
    /// - Caller must ensure no mutable references exist
    pub unsafe fn borrowed_read(ptr: *const T, validity: ValidityFlag) -> Self {
        // SAFETY: forwards this constructor's contract unchanged
        unsafe { Self::borrowed_ref(ptr, validity) }
    }

    /// Create mutable borrowed resource storage (for ResMut[T])
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in World resource storage
    /// - The pointer must remain valid while `validity` is non-Invalid
    /// - Caller must ensure exclusive mutable access
    pub unsafe fn borrowed_write(ptr: *mut T, validity: ValidityFlag) -> Self {
        // SAFETY: forwards this constructor's contract unchanged
        unsafe { Self::borrowed_mut(ptr, validity) }
    }

    /// Get immutable reference to the resource, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, StorageError> {
        match &self.inner {
            ResourceStorageInner::Owned { data, .. } => Ok(&**data),
            ResourceStorageInner::BorrowedRef(b) => b.get(),
            ResourceStorageInner::BorrowedMut(b) => b.get(),
            ResourceStorageInner::Revalidating(r) => r.get::<T>(),
        }
    }

    /// Get mutable reference to the resource, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        match &mut self.inner {
            ResourceStorageInner::Owned { data, .. } => Ok(&mut **data),
            ResourceStorageInner::BorrowedRef(_) => Err(StorageError::ReadOnly),
            ResourceStorageInner::BorrowedMut(b) => b.get_mut(),
            ResourceStorageInner::Revalidating(r) => r.get_mut::<T>(),
        }
    }

    /// Borrow a field from the resource storage
    ///
    /// Owned storage returns a read-only snapshot. Borrowed storage returns a
    /// validity-bound field inheriting the parent's read/write access.
    pub fn borrow_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: crate::BorrowableStorage<F>,
    {
        match &self.inner {
            ResourceStorageInner::Owned { data, .. } => Ok(S::snapshot(field_accessor(&**data))),
            ResourceStorageInner::BorrowedRef(b) => b.borrow_field(field_accessor),
            ResourceStorageInner::BorrowedMut(b) => b.borrow_field(field_accessor),
            ResourceStorageInner::Revalidating(r) => r.child_of::<T, F, S>(field_accessor),
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

    /// Borrow a read-only value reached through a projection that may not be an
    /// inline field of the resource.
    ///
    /// # Safety
    /// The projected value must remain at the returned address while this
    /// storage's validity flag permits access. In particular, callers must not
    /// expose an operation that can relocate a value stored behind an internal
    /// collection while the returned borrow is live.
    pub unsafe fn borrow_projected_ref<F: Clone, S>(
        &self,
        project: impl FnOnce(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: crate::BorrowableStorage<F>,
    {
        match &self.inner {
            ResourceStorageInner::Owned { data, validity } => {
                validity.check_read()?;
                let ptr = project(&**data) as *const F;
                // SAFETY: the caller guarantees the projection stays at this
                // address for the shared validity window.
                Ok(unsafe { S::borrowed_ref(ptr, validity.clone()) })
            }
            ResourceStorageInner::BorrowedRef(resource) => {
                resource.validity().check_read()?;
                let ptr = project(resource.get()?) as *const F;
                // SAFETY: forwarded from this method's projection contract.
                Ok(unsafe { S::borrowed_ref(ptr, resource.validity().clone()) })
            }
            ResourceStorageInner::BorrowedMut(resource) => {
                resource.validity().check_read()?;
                let ptr = project(resource.get()?) as *const F;
                // A shared projection is read-only even when its parent came
                // from ResMut; config_mut-style APIs use the mutable twin.
                // SAFETY: forwarded from this method's projection contract.
                Ok(unsafe { S::borrowed_ref(ptr, resource.validity().clone()) })
            }
            ResourceStorageInner::Revalidating(resource) => {
                resource.validity().check_read()?;
                let ptr = project(resource.get::<T>()?) as *const F;
                // SAFETY: forwarded from this method's projection contract.
                Ok(unsafe { S::borrowed_ref(ptr, resource.validity().clone()) })
            }
        }
    }

    /// Borrow a mutable value reached through a projection that may not be an
    /// inline field of the resource.
    ///
    /// # Safety
    /// The projected value must remain at the returned address while this
    /// storage's validity flag permits access. The caller must also ensure that
    /// no operation exposed during that window can relocate the projection.
    pub unsafe fn borrow_projected_mut<F: Clone, S>(
        &mut self,
        project: impl FnOnce(&mut T) -> &mut F,
    ) -> Result<S, StorageError>
    where
        S: crate::BorrowableStorage<F>,
    {
        match &mut self.inner {
            ResourceStorageInner::Owned { data, validity } => {
                validity.check_write()?;
                let ptr = project(&mut **data) as *mut F;
                // SAFETY: the pointer derives from the unique parent value and
                // the caller guarantees that the projection remains stable.
                Ok(unsafe { S::borrowed_mut(ptr, validity.clone()) })
            }
            ResourceStorageInner::BorrowedRef(_) => Err(StorageError::ReadOnly),
            ResourceStorageInner::BorrowedMut(resource) => {
                resource.validity().check_write()?;
                let validity = resource.validity().clone();
                let ptr = project(resource.get_mut()?) as *mut F;
                // SAFETY: the pointer derives from the checked unique parent
                // borrow and shares its validity window.
                Ok(unsafe { S::borrowed_mut(ptr, validity) })
            }
            ResourceStorageInner::Revalidating(resource) => {
                resource.validity().check_write()?;
                let validity = resource.validity().clone();
                let ptr = project(resource.get_mut::<T>()?) as *mut F;
                // SAFETY: the parent was freshly re-resolved for unique access;
                // the caller guarantees that this projection remains stable.
                Ok(unsafe { S::borrowed_mut(ptr, validity) })
            }
        }
    }

    /// Materialize an explicit read-only snapshot of a resource field.
    pub fn snapshot_field_as<F: Clone, S, W>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<W, StorageError>
    where
        S: crate::BorrowableStorage<F>,
        W: crate::FromBorrowedStorage<S>,
    {
        let resource = self.as_ref()?;
        Ok(W::from_borrowed(S::snapshot(field_accessor(resource))))
    }

    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        matches!(self.inner, ResourceStorageInner::Owned { .. })
    }

    #[allow(dead_code)]
    pub fn is_borrowed(&self) -> bool {
        matches!(
            self.inner,
            ResourceStorageInner::BorrowedRef(_)
                | ResourceStorageInner::BorrowedMut(_)
                | ResourceStorageInner::Revalidating(_)
        )
    }
}

impl<T: Resource + Clone> ResourceStorage<T> {
    /// Convert storage to owned resource, consuming self
    pub fn into_owned(self) -> Result<T, StorageError> {
        Ok(self.as_ref()?.clone())
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{
        change_detection::DetectChanges,
        prelude::{Component, Resource},
    };

    use super::*;
    use crate::{AccessMode, FieldStorage, ValidityFlag, ValidityGuard};

    #[derive(Clone, Debug, PartialEq, Resource)]
    struct TestResource {
        value: i32,
    }

    #[derive(Clone, Debug, PartialEq, Resource)]
    struct NestedResource {
        x: f32,
        y: f32,
    }

    #[derive(Clone, Debug, PartialEq, Resource)]
    struct ProjectedResource {
        value: Box<String>,
    }

    #[derive(Component)]
    struct Relocator;

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

    #[test]
    fn projected_mutation_reaches_non_inline_resource_value() {
        let mut storage = ResourceStorage::owned(ProjectedResource {
            value: Box::new("before".to_string()),
        });

        let mut projected: FieldStorage<String> =
            unsafe { storage.borrow_projected_mut(|resource| resource.value.as_mut()) }.unwrap();
        projected.as_mut().unwrap().push_str(" after");

        assert_eq!(storage.as_ref().unwrap().value.as_str(), "before after");
        drop(storage);
        assert!(matches!(
            projected.as_ref(),
            Err(StorageError::InvalidAccess)
        ));
    }

    #[test]
    fn projected_read_from_mutable_resource_stays_read_only() {
        let mut resource = ProjectedResource {
            value: Box::new("value".to_string()),
        };
        let validity = ValidityFlag::new_write();
        let storage = unsafe {
            ResourceStorage::borrowed_write(&mut resource as *mut ProjectedResource, validity)
        };

        let mut projected: FieldStorage<String> =
            unsafe { storage.borrow_projected_ref(|resource| resource.value.as_ref()) }.unwrap();

        assert_eq!(projected.as_ref().unwrap().as_str(), "value");
        assert!(matches!(projected.as_mut(), Err(StorageError::ReadOnly)));
    }

    #[test]
    fn revalidating_storage_survives_resource_entity_moves() {
        let mut world = World::new();
        world.insert_resource(TestResource { value: 42 });
        let component_id = world.components().component_id::<TestResource>().unwrap();
        let entity = world.resource_entities().get(component_id).unwrap();
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let world_ptr = &mut world as *mut World;
        let mut storage = unsafe {
            ResourceStorage::<TestResource>::revalidating(world_ptr, entity, component_id, validity)
        };

        world.entity_mut(entity).insert(Relocator);
        storage.as_mut().unwrap().value = 99;

        assert_eq!(world.resource::<TestResource>().value, 99);
    }

    #[test]
    fn revalidating_resource_write_marks_change_detection() {
        let mut world = World::new();
        world.insert_resource(TestResource { value: 42 });
        world.clear_trackers();

        let component_id = world.components().component_id::<TestResource>().unwrap();
        let entity = world.resource_entities().get(component_id).unwrap();
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let world_ptr = &mut world as *mut World;
        let mut storage = unsafe {
            ResourceStorage::<TestResource>::revalidating(world_ptr, entity, component_id, validity)
        };

        storage.as_mut().unwrap().value = 99;

        assert!(
            world
                .get_resource_ref::<TestResource>()
                .unwrap()
                .is_changed()
        );
    }

    #[test]
    fn revalidating_read_storage_rejects_mutation() {
        let mut world = World::new();
        world.insert_resource(TestResource { value: 42 });
        let component_id = world.components().component_id::<TestResource>().unwrap();
        let entity = world.resource_entities().get(component_id).unwrap();
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);
        let world_ptr = &mut world as *mut World;
        let mut storage = unsafe {
            ResourceStorage::<TestResource>::revalidating(world_ptr, entity, component_id, validity)
        };

        assert_eq!(storage.as_ref().unwrap().value, 42);
        assert!(storage.as_mut().is_err());
    }
}
