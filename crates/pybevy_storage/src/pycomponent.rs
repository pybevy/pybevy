//! Generic component storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for all PyBevy component types.
//! See the main crate's `pycomponent.rs` for detailed safety documentation.

use bevy::ecs::{
    component::{Component, ComponentId},
    entity::Entity,
    world::unsafe_world_cell::UnsafeWorldCell,
};

use crate::{
    ReadField, ReadVariant, RevalidatingSource, StorageMut, StorageRef, ValidateField, WriteField,
    WriteVariant,
    borrowed::{BorrowedMut, BorrowedRef, RevalidatingField},
    component_change::ComponentWriteContext,
    owned_cell::OwnedCell,
    storage_error::StorageError,
    storage_traits::{BorrowableStorage, FromBorrowedStorage},
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
        data: OwnedCell<T>,
        validity: ValidityFlag,
    },

    /// Read-only snapshot of a component-shaped field extracted from owned
    /// asset storage.
    OwnedReadOnly {
        data: OwnedCell<T>,
        validity: ValidityFlag,
    },

    /// Read-only borrow into ECS storage
    BorrowedRef(BorrowedRef<T>),

    /// Mutable borrow into ECS storage
    BorrowedMut(BorrowedMut<T>),

    /// A component reached from a long-lived `world.get`/`world.get_mut` handle.
    /// Caches no pointer: it re-derives the component's current address on every
    /// access (offset 0), so it stays valid across structural moves that relocate the
    /// component and errors after the entity is despawned. Boxed so this variant does
    /// not enlarge `ComponentStorage`.
    Revalidating(Box<RevalidatingField>),

    /// A native component value nested inside a live Bevy asset.
    Source(RevalidatingSource<T>),
}

impl<T: Component> Drop for ComponentStorage<T> {
    fn drop(&mut self) {
        match &self.inner {
            ComponentStorageInner::Owned { validity, .. }
            | ComponentStorageInner::OwnedReadOnly { validity, .. } => validity.set_invalid(),
            _ => {}
        }
    }
}

impl<T: Component + PartialEq> PartialEq for ComponentStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(left), Ok(right)) => left.reborrow() == right.reborrow(),
            _ => false,
        }
    }
}

impl<T: Component> ComponentStorage<T> {
    /// Create owned component storage with validity tracking
    pub fn owned(component: T) -> Self {
        Self {
            inner: ComponentStorageInner::Owned {
                data: OwnedCell::new(component),
                validity: ValidityFlag::new_write(),
            },
        }
    }

    /// Create an enforced read-only snapshot.
    pub fn read_only_snapshot(component: &T) -> Self
    where
        T: Clone,
    {
        Self {
            inner: ComponentStorageInner::OwnedReadOnly {
                data: OwnedCell::new(component.clone()),
                validity: ValidityFlag::new_read(),
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

    /// Create mutable query storage that marks the component only on a real write.
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
            inner: ComponentStorageInner::BorrowedMut(unsafe {
                BorrowedMut::new_tracked(ptr, validity, context)
            }),
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

    /// Create a re-resolving component handle keyed by ECS identity rather than a
    /// cached pointer.
    ///
    /// Unlike `borrowed`, this caches no pointer: each access re-derives the
    /// component's current base address from `(world, entity, component_id)`, so it
    /// stays valid across structural mutations that relocate the component and errors
    /// once the entity is despawned. Used for a component escaped from a long-lived
    /// `world.get`/`world.get_mut` handle, where a cached borrow would dangle.
    ///
    /// # Safety
    /// - `world` must remain live while `validity` is active.
    /// - The cell must permit component access matching the validity mode.
    /// - `(entity, component_id)` must identify a live component of type `T`.
    pub unsafe fn revalidating(
        world: UnsafeWorldCell<'_>,
        entity: Entity,
        component_id: ComponentId,
        validity: ValidityFlagWithMode,
    ) -> Self {
        Self {
            // SAFETY: whole-component handle, so offset is 0; forwards the contract
            inner: ComponentStorageInner::Revalidating(Box::new(unsafe {
                RevalidatingField::new(world, entity, component_id, 0, validity)
            })),
        }
    }

    pub fn revalidating_source(source: RevalidatingSource<T>) -> Self {
        Self {
            inner: ComponentStorageInner::Source(source),
        }
    }

    /// Get immutable reference to the component, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<StorageRef<'_, T>, StorageError> {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. }
            | ComponentStorageInner::OwnedReadOnly { data, .. } => {
                Ok(StorageRef::Direct(data.get()))
            }
            ComponentStorageInner::BorrowedRef(b) => b.get().map(StorageRef::Direct),
            ComponentStorageInner::BorrowedMut(b) => b.get().map(StorageRef::Direct),
            ComponentStorageInner::Revalidating(r) => r.get::<T>().map(StorageRef::Direct),
            ComponentStorageInner::Source(source) => source.resolve_ref().map(StorageRef::Source),
        }
    }

    /// Get mutable reference to the component, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<StorageMut<'_, T>, StorageError> {
        match &mut self.inner {
            ComponentStorageInner::Owned { data, .. } => Ok(StorageMut::Direct(data.get_mut())),
            ComponentStorageInner::OwnedReadOnly { .. } => Err(StorageError::OwnedFieldReadOnly),
            ComponentStorageInner::BorrowedRef(borrow) => {
                borrow.validity().check_read()?;
                Err(StorageError::ReadOnly)
            }
            ComponentStorageInner::BorrowedMut(b) => b.get_mut().map(StorageMut::Direct),
            ComponentStorageInner::Revalidating(r) => r.get_mut::<T>().map(StorageMut::Direct),
            ComponentStorageInner::Source(source) => source.resolve_mut().map(StorageMut::Source),
        }
    }

    /// Mutate the value, staging the write through any field validator first.
    ///
    /// Wrappers that can sit under a validated field write through this
    /// instead of `as_mut`, which rejects validated paths. `mutate` must depend
    /// only on the value it receives.
    pub fn update<R>(&mut self, mut mutate: impl FnMut(&mut T) -> R) -> Result<R, StorageError> {
        match &self.inner {
            ComponentStorageInner::Source(source) => source.update(mutate),
            _ => Ok(mutate(self.as_mut()?.reborrow_mut())),
        }
    }

    /// Borrow a field whose every nested write must pass `validate`.
    ///
    /// Borrowed storage yields a checked path that stages each child write on a
    /// copy of the field and commits it only when `validate` accepts the
    /// result, so a rejected write neither changes the value nor marks the
    /// owner changed. Owned storage yields a read-only snapshot.
    pub fn borrow_validated_field_as<F: Clone + 'static, S, W>(
        &self,
        read: ReadField<T, F>,
        write: WriteField<T, F>,
        validate: ValidateField<F>,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        let source = match &self.inner {
            ComponentStorageInner::Source(source) => {
                read(&*source.resolve_ref()?);
                source.validated_field(read, write, validate)
            }
            _ => {
                read(&*self.as_ref()?);
                match RevalidatingSource::from_borrowed_component(self) {
                    Some(source) => source.validated_field(read, write, validate),
                    None => return self.snapshot_field_as(read),
                }
            }
        };
        Ok(W::from_borrowed(S::revalidating_source(source)))
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
            // A re-resolving handle shares its ECS identity, so the sub-object also
            // re-resolves from the entity (e.g. AnimationPlayer.play() -> ActiveAnimation).
            ComponentStorageInner::Revalidating(r) => {
                ComponentStorageInner::Revalidating(r.clone())
            }
            ComponentStorageInner::Source(source) => ComponentStorageInner::Source(source.clone()),
            ComponentStorageInner::Owned { data, validity } => {
                // Write permission has to come from the cell: a pointer cast from
                // `&T` could never be written through, whatever its lifetime.
                let ptr = data.as_mut_ptr();
                // SAFETY: ptr names our own cell, valid while this storage lives;
                // the shared flag is invalidated by Drop before the cell is freed.
                ComponentStorageInner::BorrowedMut(unsafe {
                    BorrowedMut::new(ptr, validity.clone())
                })
            }
            ComponentStorageInner::OwnedReadOnly { data, validity } => {
                let ptr = data.as_ptr();
                // SAFETY: the pointer names this storage's cell and the shared
                // flag is invalidated before that cell is dropped.
                ComponentStorageInner::BorrowedRef(unsafe {
                    BorrowedRef::new(ptr, validity.clone())
                })
            }
        };
        Self { inner }
    }

    /// Create a read-only handle to the same component data.
    pub fn share_borrow_ref(&self) -> Self {
        let inner = match &self.inner {
            ComponentStorageInner::BorrowedRef(b) => ComponentStorageInner::BorrowedRef(b.clone()),
            ComponentStorageInner::BorrowedMut(b) => {
                ComponentStorageInner::BorrowedRef(b.clone_as_ref())
            }
            ComponentStorageInner::Revalidating(r) => {
                ComponentStorageInner::Revalidating(Box::new(r.clone_as_ref()))
            }
            ComponentStorageInner::Source(source) => ComponentStorageInner::Source(source.clone()),
            ComponentStorageInner::Owned { data, validity }
            | ComponentStorageInner::OwnedReadOnly { data, validity } => {
                let ptr = data.as_ptr();
                // SAFETY: the pointer names this storage's cell and the shared
                // flag is invalidated before that cell is dropped.
                ComponentStorageInner::BorrowedRef(unsafe {
                    BorrowedRef::new(ptr, validity.clone())
                })
            }
        };

        Self { inner }
    }

    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        matches!(
            self.inner,
            ComponentStorageInner::Owned { .. } | ComponentStorageInner::OwnedReadOnly { .. }
        )
    }

    #[allow(dead_code)]
    pub fn is_borrowed(&self) -> bool {
        matches!(
            self.inner,
            ComponentStorageInner::BorrowedRef(_)
                | ComponentStorageInner::BorrowedMut(_)
                | ComponentStorageInner::Revalidating(_)
                | ComponentStorageInner::Source(_)
        )
    }

    /// Borrow a field from the component storage
    ///
    /// Owned storage returns a read-only snapshot. Borrowed storage returns a
    /// validity-bound field inheriting the parent's read/write access.
    pub fn borrow_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. }
            | ComponentStorageInner::OwnedReadOnly { data, .. } => {
                Ok(S::snapshot(field_accessor(data.get())))
            }
            ComponentStorageInner::BorrowedRef(b) => b.borrow_field(field_accessor),
            ComponentStorageInner::BorrowedMut(b) => b.borrow_field(field_accessor),
            ComponentStorageInner::Revalidating(r) => r.child_of::<T, F, S>(field_accessor),
            ComponentStorageInner::Source(source) => {
                let resolved = source.resolve_ref()?;
                Ok(S::snapshot(field_accessor(&resolved)))
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
        W: FromBorrowedStorage<S>,
    {
        Ok(W::from_borrowed(self.borrow_field(field_accessor)?))
    }

    /// Materialize an explicit read-only snapshot of a component field.
    pub fn snapshot_field_as<F: Clone, S, W>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        let component = self.as_ref()?;
        Ok(W::from_borrowed(S::snapshot(field_accessor(&component))))
    }

    /// Borrow a field through a paired path when this component-shaped value
    /// is nested in an asset.
    pub fn borrow_resolved_field_as<F: Clone + 'static, S, W>(
        &self,
        read: ReadField<T, F>,
        write: WriteField<T, F>,
    ) -> Result<W, StorageError>
    where
        T: 'static,
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        let storage = match &self.inner {
            ComponentStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current);
                drop(current);
                S::revalidating_source(source.field(read, write))
            }
            _ => return self.borrow_field_as(read),
        };
        Ok(W::from_borrowed(storage))
    }

    /// Borrow an enum payload that rechecks the expected variant on every access.
    ///
    /// Borrowed storage yields a checked path rooted at the live component, so a
    /// payload escaped before the variant is replaced errors instead of reading or
    /// writing through a stale offset. Owned storage yields a read-only snapshot.
    pub fn borrow_resolved_variant_as<F: Clone + 'static, S, W>(
        &self,
        name: &'static str,
        read: ReadVariant<T, F>,
        write: WriteVariant<T, F>,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        let storage = match &self.inner {
            ComponentStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current).ok_or(StorageError::VariantChanged(name))?;
                drop(current);
                S::revalidating_source(source.variant(name, read, write))
            }
            _ => {
                let current = self.as_ref()?;
                let value = read(&current).ok_or(StorageError::VariantChanged(name))?;
                match RevalidatingSource::from_borrowed_component(self) {
                    Some(source) => S::revalidating_source(source.variant(name, read, write)),
                    None => S::snapshot(value),
                }
            }
        };
        Ok(W::from_borrowed(storage))
    }

    /// Borrow an `Option` field whose child rechecks `Some` on every access.
    ///
    /// Borrowed storage yields a checked path rooted at the live component, so a
    /// child escaped before the field is set to `None` errors instead of reading or
    /// writing through a stale offset, and follows a later `Some` value. Owned
    /// storage yields a read-only snapshot.
    pub fn borrow_optional_field<F: Clone + 'static, S>(
        &self,
        name: &'static str,
        read: ReadField<T, Option<F>>,
        write: WriteField<T, Option<F>>,
    ) -> Result<Option<S>, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        let child = match &self.inner {
            ComponentStorageInner::Source(source) => {
                if read(&*source.resolve_ref()?).is_none() {
                    return Ok(None);
                }
                source.optional(name, read, write)
            }
            _ => {
                let current = self.as_ref()?;
                let Some(value) = read(&current) else {
                    return Ok(None);
                };
                match RevalidatingSource::from_borrowed_component(self) {
                    Some(source) => source.optional(name, read, write),
                    None => return Ok(Some(S::snapshot(value))),
                }
            }
        };
        Ok(Some(S::revalidating_source(child)))
    }
}

impl<T: Component> BorrowableStorage<T> for ComponentStorage<T> {
    unsafe fn borrowed_ref(ptr: *const T, validity: ValidityFlag) -> Self {
        // SAFETY: forwards this constructor's contract unchanged.
        unsafe { Self::borrowed_ref(ptr, validity) }
    }

    unsafe fn borrowed_mut(ptr: *mut T, validity: ValidityFlag) -> Self {
        // SAFETY: forwards this constructor's contract unchanged.
        unsafe { Self::borrowed_mut(ptr, validity) }
    }

    unsafe fn borrowed_mut_tracked(
        ptr: *mut T,
        validity: ValidityFlag,
        context: ComponentWriteContext,
    ) -> Self {
        // SAFETY: forwards this constructor's contract unchanged.
        unsafe { Self::borrowed_mut_tracked(ptr, validity, context) }
    }

    fn snapshot(value: &T) -> Self
    where
        T: Clone,
    {
        Self::read_only_snapshot(value)
    }

    unsafe fn revalidating_field(
        world: UnsafeWorldCell<'_>,
        entity: Entity,
        component_id: ComponentId,
        offset: usize,
        validity: ValidityFlagWithMode,
    ) -> Self {
        assert_eq!(offset, 0, "whole component paths require offset zero");
        // SAFETY: forwards this constructor's world and component identity
        // contract unchanged.
        unsafe { Self::revalidating(world, entity, component_id, validity) }
    }

    fn revalidating_source(source: RevalidatingSource<T>) -> Self
    where
        T: 'static,
    {
        Self::revalidating_source(source)
    }
}

impl<T: Component + Clone> ComponentStorage<T> {
    /// Convert storage to owned component, consuming self
    pub fn into_owned(self) -> Result<T, StorageError> {
        Ok(self.as_ref()?.clone())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use bevy::ecs::{
        change_detection::{DetectChangesMut, Tick},
        component::Component,
        entity::Entity,
        world::World,
    };

    use super::*;
    use crate::{
        AccessMode, BorrowableStorage, ComponentWriteContext, ValidityFlag, ValidityGuard,
        storage_traits::FromBorrowedStorage, value_storage::ValueStorage,
    };

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
    fn equality_compares_live_values_and_never_reads_expired_borrows() {
        let component = TestComponent { value: 42 };
        let validity = ValidityFlag::new_read();
        // SAFETY: component stays alive while the borrow is valid.
        let borrowed = unsafe {
            ComponentStorage::borrowed_ref(&component as *const TestComponent, validity.clone())
        };
        let equal_owned = ComponentStorage::owned(TestComponent { value: 42 });
        let unequal_owned = ComponentStorage::owned(TestComponent { value: 7 });

        {
            let _guard = ValidityGuard::new(validity.clone());
            assert_eq!(borrowed, equal_owned);
            assert_eq!(equal_owned, borrowed);
            assert_ne!(borrowed, unequal_owned);
            std::thread::scope(|scope| {
                assert!(!scope.spawn(|| borrowed == equal_owned).join().unwrap());
            });
        }

        assert_ne!(borrowed, equal_owned);
    }

    #[test]
    fn component_snapshot_is_read_only() {
        let value = TestComponent { value: 42 };
        let mut storage =
            <ComponentStorage<TestComponent> as BorrowableStorage<TestComponent>>::snapshot(&value);

        assert_eq!(storage.as_ref().unwrap().value, 42);
        assert!(matches!(
            storage.as_mut(),
            Err(StorageError::OwnedFieldReadOnly)
        ));
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
        let storage = ComponentStorage::owned(NestedComponent { x: 1.0, y: 2.0 });
        let field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();

        // Should be an OwnedReadOnly snapshot, not a borrowed pointer
        assert!(field.is_owned_read_only());
        assert_eq!(field.get().unwrap(), 1.0);
    }

    #[test]
    fn test_owned_borrow_field_snapshot_rejects_writes() {
        let storage = ComponentStorage::owned(NestedComponent { x: 1.0, y: 2.0 });
        let mut field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();

        assert!(matches!(
            field.as_mut(),
            Err(crate::StorageError::OwnedFieldReadOnly)
        ));
    }

    #[test]
    fn test_owned_borrow_field_snapshot_survives_parent_drop() {
        let field: ValueStorage<f32>;
        {
            let storage = ComponentStorage::owned(NestedComponent { x: 42.0, y: 0.0 });
            field = storage.borrow_field(|c| &c.x).unwrap();
            // storage dropped here
        }
        // Snapshot is independent: still readable after parent is dropped
        assert_eq!(field.get().unwrap(), 42.0);
    }

    #[test]
    fn test_mutably_borrowed_field_stays_live() {
        let mut component = NestedComponent { x: 5.0, y: 6.0 };
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let storage =
            unsafe { ComponentStorage::borrowed(&mut component as *mut NestedComponent, validity) };

        let field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();
        assert!(field.is_borrowed());
    }

    #[test]
    fn test_read_only_borrowed_field_stays_validity_bound() {
        let component = NestedComponent { x: 5.0, y: 6.0 };
        let validity = ValidityFlag::new_read();
        let storage = unsafe {
            ComponentStorage::borrowed_ref(&component as *const NestedComponent, validity.clone())
        };

        let mut field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();
        assert!(field.is_borrowed());
        assert!(matches!(field.as_mut(), Err(StorageError::ReadOnly)));
        validity.set_invalid();
        assert!(matches!(field.get(), Err(StorageError::InvalidAccess)));
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
    fn borrowable_storage_trait_forwards_to_inherent_constructors() {
        let mut component = TestComponent { value: 1 };

        // SAFETY: `component` outlives the borrowed storage for this test.
        let read_only = unsafe {
            <ComponentStorage<TestComponent> as BorrowableStorage<TestComponent>>::borrowed_ref(
                &component as *const TestComponent,
                ValidityFlag::new_read(),
            )
        };
        assert_eq!(read_only.as_ref().unwrap().value, 1);

        let validity = ValidityFlag::new_write();
        // SAFETY: `component` outlives the borrow and is not aliased here.
        let mut writable = unsafe {
            <ComponentStorage<TestComponent> as BorrowableStorage<TestComponent>>::borrowed_mut(
                &mut component as *mut TestComponent,
                validity.clone(),
            )
        };
        writable.as_mut().unwrap().value = 5;
        assert_eq!(component.value, 5);

        let mut world = World::new();
        let entity = world.spawn(TestComponent { value: 9 }).id();
        let component_id = world.component_id::<TestComponent>().unwrap();
        // SAFETY: `world` outlives the revalidating storage for this test.
        let revalidating = unsafe {
            <ComponentStorage<TestComponent> as BorrowableStorage<TestComponent>>::revalidating_field(
                world.as_unsafe_world_cell(),
                entity,
                component_id,
                0,
                validity.with_access_mode(AccessMode::Write),
            )
        };
        assert_eq!(revalidating.as_ref().unwrap().value, 9);
        drop(revalidating);

        let mut world = World::new();
        let entity = world.spawn(TestComponent { value: 5 }).id();
        let component_id = world.component_id::<TestComponent>().unwrap();
        let last_run = world.read_change_tick();
        world.increment_change_tick();
        world.increment_change_tick();
        let this_run = world.read_change_tick();

        let ptr = {
            let mut component = world.get_mut::<TestComponent>(entity).unwrap();
            component.bypass_change_detection() as *mut TestComponent
        };
        // SAFETY: `world` outlives the context and no competing access occurs.
        let context = unsafe {
            ComponentWriteContext::new_with_offset(
                world.as_unsafe_world_cell(),
                entity,
                component_id,
                0,
                last_run,
                this_run,
            )
        };
        // SAFETY: `ptr` and `context` identify the same live World component.
        let mut tracked = unsafe {
            <ComponentStorage<TestComponent> as BorrowableStorage<TestComponent>>::borrowed_mut_tracked(
                ptr,
                validity,
                context,
            )
        };

        let ticks = world
            .entity(entity)
            .get_change_ticks_by_id(component_id)
            .unwrap();
        assert!(!ticks.is_changed(last_run, this_run));

        tracked.as_mut().unwrap().value = 42;
        assert_eq!(
            world.entity(entity).get::<TestComponent>().unwrap().value,
            42
        );
        let ticks = world
            .entity(entity)
            .get_change_ticks_by_id(component_id)
            .unwrap();
        assert!(ticks.is_changed(last_run, this_run));
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
    fn test_share_borrow_from_owned_interleaves_with_parent_writes() {
        // Mirrors AnimationPlayer.play(): the sub-object keeps writing through
        // its shared pointer after the owner has written through its own handle.
        let mut owner = ComponentStorage::owned(TestComponent { value: 1 });
        let mut shared = owner.share_borrow();

        shared.as_mut().unwrap().value = 2;
        assert_eq!(owner.as_ref().unwrap().value, 2);

        owner.as_mut().unwrap().value = 3;
        assert_eq!(shared.as_ref().unwrap().value, 3);

        shared.as_mut().unwrap().value += 4;
        assert_eq!(owner.as_ref().unwrap().value, 7);
        assert_eq!(shared.as_ref().unwrap().value, 7);
    }

    #[test]
    fn test_share_borrow_from_owned_expires_with_the_owner() {
        let owner = ComponentStorage::owned(TestComponent { value: 5 });
        let mut shared = owner.share_borrow();
        assert_eq!(shared.as_ref().unwrap().value, 5);

        drop(owner);

        assert!(matches!(shared.as_ref(), Err(StorageError::InvalidAccess)));
        assert!(matches!(shared.as_mut(), Err(StorageError::InvalidAccess)));
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
    fn test_share_borrow_ref_downgrades_write_mode() {
        let validity = ValidityFlag::new_write();
        let mut value = TestComponent { value: 42 };
        // SAFETY: value outlives both storage handles in this test.
        let storage =
            unsafe { ComponentStorage::borrowed_mut(&mut value as *mut TestComponent, validity) };
        let mut shared = storage.share_borrow_ref();

        assert_eq!(shared.as_ref().unwrap().value, 42);
        assert!(matches!(shared.as_mut(), Err(StorageError::ReadOnly)));
    }

    #[test]
    fn test_borrow_field_inherits_parent_access() {
        // Mutable parent -> mutable child field.
        let mut component = NestedComponent { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_write();
        let storage = unsafe {
            ComponentStorage::borrowed_mut(&mut component as *mut NestedComponent, validity)
        };
        let mut field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();
        *field.as_mut().unwrap() = 9.0;
        assert_eq!(component.x, 9.0);

        // Read-only parent -> live read-only child borrow.
        let component = NestedComponent { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_read();
        let storage = unsafe {
            ComponentStorage::borrowed_ref(&component as *const NestedComponent, validity)
        };
        let mut field: ValueStorage<f32> = storage.borrow_field(|c| &c.x).unwrap();
        assert!(matches!(field.as_mut(), Err(StorageError::ReadOnly)));
    }

    #[test]
    fn test_revalidating_reads_and_writes_live_component() {
        let mut world = World::new();
        let cid = world.register_component::<TestComponent>();
        let e = world.spawn(TestComponent { value: 5 }).id();
        let cell = world.as_unsafe_world_cell();

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let mut storage =
            unsafe { ComponentStorage::<TestComponent>::revalidating(cell, e, cid, validity) };

        assert!(storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().value, 5);
        storage.as_mut().unwrap().value = 99;
        assert_eq!(world.entity(e).get::<TestComponent>().unwrap().value, 99);
    }

    #[test]
    fn test_revalidating_tracks_after_archetype_move() {
        let mut world = World::new();
        let cid = world.register_component::<TestComponent>();
        let e = world.spawn(TestComponent { value: 1 }).id();
        let cell = world.as_unsafe_world_cell();

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let mut storage =
            unsafe { ComponentStorage::<TestComponent>::revalidating(cell, e, cid, validity) };

        // Insert a second component: relocates the entity, dangling a cached pointer.
        world
            .entity_mut(e)
            .insert(NestedComponent { x: 0.0, y: 0.0 });

        assert_eq!(storage.as_ref().unwrap().value, 1);
        storage.as_mut().unwrap().value = 7;
        assert_eq!(world.entity(e).get::<TestComponent>().unwrap().value, 7);
    }

    #[test]
    fn test_revalidating_errors_after_despawn() {
        let mut world = World::new();
        let cid = world.register_component::<TestComponent>();
        let e = world.spawn(TestComponent { value: 5 }).id();
        let cell = world.as_unsafe_world_cell();

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let mut storage =
            unsafe { ComponentStorage::<TestComponent>::revalidating(cell, e, cid, validity) };

        assert!(storage.as_ref().is_ok());
        world.despawn(e);
        assert!(storage.as_ref().is_err());
        assert!(storage.as_mut().is_err());
    }

    #[test]
    fn test_revalidating_read_mode_rejects_write() {
        let mut world = World::new();
        let cid = world.register_component::<TestComponent>();
        let e = world.spawn(TestComponent { value: 5 }).id();
        let cell = world.as_unsafe_world_cell();

        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);
        let mut storage =
            unsafe { ComponentStorage::<TestComponent>::revalidating(cell, e, cid, validity) };

        assert!(storage.as_ref().is_ok());
        assert!(matches!(storage.as_mut(), Err(StorageError::ReadOnly)));
    }

    #[test]
    fn test_revalidating_borrow_field_writes_through_and_tracks_move() {
        let mut world = World::new();
        let cid = world.register_component::<NestedComponent>();
        let e = world.spawn(NestedComponent { x: 1.0, y: 2.0 }).id();
        let cell = world.as_unsafe_world_cell();

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let storage =
            unsafe { ComponentStorage::<NestedComponent>::revalidating(cell, e, cid, validity) };

        // A sub-field of a re-resolving component re-resolves too (composed offset).
        let mut field: ValueStorage<f32> = storage.borrow_field(|c| &c.y).unwrap();
        assert_eq!(field.get().unwrap(), 2.0);

        // Force an archetype move; the field handle must still land on the live component.
        world.entity_mut(e).insert(TestComponent { value: 7 });
        *field.as_mut().unwrap() = 42.0;
        assert_eq!(world.entity(e).get::<NestedComponent>().unwrap().y, 42.0);
    }

    #[test]
    fn test_share_borrow_revalidating_tracks_move() {
        // Mirrors AnimationPlayer.play() sharing a world.get_mut handle: the shared
        // sub-object must re-resolve from the entity, not a cached pointer.
        let mut world = World::new();
        let cid = world.register_component::<TestComponent>();
        let e = world.spawn(TestComponent { value: 1 }).id();
        let cell = world.as_unsafe_world_cell();

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let storage =
            unsafe { ComponentStorage::<TestComponent>::revalidating(cell, e, cid, validity) };
        let mut shared = storage.share_borrow();
        assert!(shared.is_borrowed());

        world
            .entity_mut(e)
            .insert(NestedComponent { x: 0.0, y: 0.0 });
        shared.as_mut().unwrap().value = 55;
        assert_eq!(storage.as_ref().unwrap().value, 55);
        assert_eq!(world.entity(e).get::<TestComponent>().unwrap().value, 55);
    }
    #[derive(Clone, Debug, PartialEq, Component)]
    enum Shape {
        Size([u32; 2]),
        Handle(u64),
    }

    struct Payload(ValueStorage<[u32; 2]>);

    impl FromBorrowedStorage<ValueStorage<[u32; 2]>> for Payload {
        fn from_borrowed(storage: ValueStorage<[u32; 2]>) -> Self {
            Self(storage)
        }
    }

    fn size_of(shape: &Shape) -> Option<&[u32; 2]> {
        match shape {
            Shape::Size(size) => Some(size),
            _ => None,
        }
    }

    fn size_of_mut(shape: &mut Shape) -> Option<&mut [u32; 2]> {
        match shape {
            Shape::Size(size) => Some(size),
            _ => None,
        }
    }

    #[test]
    fn test_revalidating_variant_payload_rejects_stale_variant() {
        let mut world = World::new();
        let cid = world.register_component::<Shape>();
        let e = world.spawn(Shape::Size([1, 2])).id();
        let cell = world.as_unsafe_world_cell();
        let validity = ValidityFlag::new_write();
        // SAFETY: the test owns World and accesses only this Shape through the handle.
        let storage = unsafe {
            ComponentStorage::<Shape>::revalidating(
                cell,
                e,
                cid,
                validity.with_access_mode(AccessMode::Write),
            )
        };
        let Payload(mut payload) = storage
            .borrow_resolved_variant_as("Shape.Size", size_of, size_of_mut)
            .unwrap();

        world.entity_mut(e).insert(TestComponent { value: 0 });
        payload.as_mut().unwrap()[0] = 7;
        assert_eq!(world.get::<Shape>(e), Some(&Shape::Size([7, 2])));

        world.entity_mut(e).insert(Shape::Handle(99));
        let before = world.entity(e).get_change_ticks::<Shape>().unwrap().changed;
        world.increment_change_tick();
        assert!(matches!(
            payload.as_ref(),
            Err(StorageError::VariantChanged("Shape.Size"))
        ));
        assert!(matches!(
            payload.as_mut(),
            Err(StorageError::VariantChanged("Shape.Size"))
        ));
        let after = world.entity(e).get_change_ticks::<Shape>().unwrap().changed;
        assert_eq!(before, after);
        assert_eq!(world.get::<Shape>(e), Some(&Shape::Handle(99)));
        assert!(matches!(
            storage.borrow_resolved_variant_as::<_, _, Payload>("Shape.Size", size_of, size_of_mut),
            Err(StorageError::VariantChanged("Shape.Size"))
        ));
        validity.set_invalid();
    }

    #[test]
    fn test_owned_variant_payload_is_read_only_snapshot() {
        let storage = ComponentStorage::owned(Shape::Size([3, 4]));
        let Payload(mut payload) = storage
            .borrow_resolved_variant_as("Shape.Size", size_of, size_of_mut)
            .unwrap();
        assert_eq!(payload.get().unwrap(), [3, 4]);
        assert!(matches!(
            payload.as_mut(),
            Err(StorageError::OwnedFieldReadOnly)
        ));
    }

    #[derive(Clone, Debug, PartialEq, Component)]
    struct Sized2 {
        size: Option<[f32; 2]>,
    }

    fn size_field(value: &Sized2) -> &Option<[f32; 2]> {
        &value.size
    }

    fn size_field_mut(value: &mut Sized2) -> &mut Option<[f32; 2]> {
        &mut value.size
    }

    fn size_changed(world: &World, entity: Entity, last_run: Tick, this_run: Tick) -> bool {
        world
            .entity(entity)
            .get_change_ticks::<Sized2>()
            .unwrap()
            .is_changed(last_run, this_run)
    }

    #[test]
    fn test_tracked_optional_child_rejects_none_and_follows_new_some() {
        let mut world = World::new();
        let entity = world
            .spawn(Sized2 {
                size: Some([1.0, 2.0]),
            })
            .id();
        let component_id = world.component_id::<Sized2>().unwrap();
        let last_run = world.read_change_tick();
        world.increment_change_tick();
        world.increment_change_tick();
        let this_run = world.read_change_tick();
        let ptr = {
            let mut component = world.get_mut::<Sized2>(entity).unwrap();
            component.bypass_change_detection() as *mut Sized2
        };
        let validity = ValidityFlag::new_write();
        // SAFETY: `world` outlives the context and no competing access occurs.
        let context = unsafe {
            ComponentWriteContext::new(
                world.as_unsafe_world_cell(),
                entity,
                component_id,
                last_run,
                this_run,
            )
        };
        // SAFETY: `ptr` and `context` identify the same live World component.
        let mut parent = unsafe {
            <ComponentStorage<Sized2> as BorrowableStorage<Sized2>>::borrowed_mut_tracked(
                ptr,
                validity.clone(),
                context,
            )
        };
        let mut child: ValueStorage<[f32; 2]> = parent
            .borrow_optional_field("Sized2.size", size_field, size_field_mut)
            .unwrap()
            .unwrap();

        assert_eq!(child.get().unwrap(), [1.0, 2.0]);
        assert!(!size_changed(&world, entity, last_run, this_run));

        world
            .get_mut::<Sized2>(entity)
            .unwrap()
            .bypass_change_detection()
            .size = None;
        assert!(matches!(
            child.get(),
            Err(StorageError::VariantChanged("Sized2.size"))
        ));
        assert!(matches!(
            child.as_mut(),
            Err(StorageError::VariantChanged("Sized2.size"))
        ));
        assert!(!size_changed(&world, entity, last_run, this_run));
        assert_eq!(world.get::<Sized2>(entity).unwrap().size, None);
        assert!(
            parent
                .borrow_optional_field::<_, ValueStorage<[f32; 2]>>(
                    "Sized2.size",
                    size_field,
                    size_field_mut
                )
                .unwrap()
                .is_none()
        );

        parent.as_mut().unwrap().size = Some([5.0, 6.0]);
        child.as_mut().unwrap()[0] = 7.0;
        assert_eq!(world.get::<Sized2>(entity).unwrap().size, Some([7.0, 6.0]));
        assert!(size_changed(&world, entity, last_run, this_run));
        validity.set_invalid();
    }

    #[test]
    fn test_revalidating_optional_child_rejects_none_after_move() {
        let mut world = World::new();
        let component_id = world.register_component::<Sized2>();
        let entity = world
            .spawn(Sized2 {
                size: Some([1.0, 2.0]),
            })
            .id();
        let validity = ValidityFlag::new_write();
        // SAFETY: the test owns World and accesses only this Sized2 through the handle.
        let parent = unsafe {
            ComponentStorage::<Sized2>::revalidating(
                world.as_unsafe_world_cell(),
                entity,
                component_id,
                validity.with_access_mode(AccessMode::Write),
            )
        };
        let mut child: ValueStorage<[f32; 2]> = parent
            .borrow_optional_field("Sized2.size", size_field, size_field_mut)
            .unwrap()
            .unwrap();

        world.entity_mut(entity).insert(TestComponent { value: 0 });
        child.as_mut().unwrap()[1] = 9.0;
        assert_eq!(world.get::<Sized2>(entity).unwrap().size, Some([1.0, 9.0]));

        world.entity_mut(entity).insert(Sized2 { size: None });
        assert!(matches!(
            child.get(),
            Err(StorageError::VariantChanged("Sized2.size"))
        ));
        assert!(matches!(
            child.as_mut(),
            Err(StorageError::VariantChanged("Sized2.size"))
        ));
        assert_eq!(world.get::<Sized2>(entity).unwrap().size, None);
        validity.set_invalid();
    }

    #[test]
    fn test_read_only_optional_child_rejects_writes() {
        let component = Sized2 {
            size: Some([3.0, 4.0]),
        };
        let validity = ValidityFlag::new_read();
        // SAFETY: component outlives every access below and is never mutated.
        let parent = unsafe { ComponentStorage::borrowed_ref(&component, validity.clone()) };
        let mut child: ValueStorage<[f32; 2]> = parent
            .borrow_optional_field("Sized2.size", size_field, size_field_mut)
            .unwrap()
            .unwrap();
        assert_eq!(child.get().unwrap(), [3.0, 4.0]);
        assert!(matches!(child.as_mut(), Err(StorageError::ReadOnly)));
        validity.set_invalid();
        assert!(matches!(child.get(), Err(StorageError::InvalidAccess)));
    }
}
