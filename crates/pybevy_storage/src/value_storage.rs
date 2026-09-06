//! Generic value storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for PyBevy value types
//! (Vec2, Vec3, Vec4, Quat, Mat3, Mat4, LinearRgba, etc.), eliminating code
//! duplication across math and color types.
//!
//! Key difference from ComponentStorage:
//! - ValueStorage stores Copy types directly (Owned(T))
//! - ComponentStorage stores larger types in Box (Owned(Box<T>))

use bevy::ecs::{component::ComponentId, entity::Entity, world::World};

use crate::{
    ReadField, ReadVariant, RevalidatingSource, StorageMut, StorageRef, WriteField, WriteVariant,
    borrowed::{BorrowedMut, BorrowedRef, RevalidatingField},
    component_change::ComponentWriteContext,
    storage_error::StorageError,
    storage_traits::{BorrowableStorage, FromBorrowedStorage},
    validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode},
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

    /// Read-only borrow into a component field
    BorrowedRef(BorrowedRef<T>),

    /// Mutable borrow into a component field
    BorrowedMut(BorrowedMut<T>),

    /// A field of a wrapper component reached from a long-lived `world.get`/`world.get_mut`
    /// proxy. Caches no pointer: it re-derives the component's current address on every
    /// access (the entity may be structurally moved while the handle is held), so
    /// reads/writes land on the live component and access after despawn errors.
    ///
    /// Boxed so this rarely-used variant does not enlarge `ValueStorage` (hence every
    /// math/color type) with its extra identity fields.
    Revalidating(Box<RevalidatingField>),

    /// A typed path into a live Bevy asset, re-resolved on every access.
    Source(Box<RevalidatingSource<T>>),
}

impl<T: Copy> Clone for ValueStorage<T> {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            ValueStorageInner::Owned(value) => ValueStorageInner::Owned(*value),
            ValueStorageInner::OwnedReadOnly(value) => ValueStorageInner::OwnedReadOnly(*value),
            ValueStorageInner::BorrowedRef(b) => ValueStorageInner::BorrowedRef(b.clone()),
            // A cloned mutable borrow downgrades to read-only to avoid aliasing.
            ValueStorageInner::BorrowedMut(b) => ValueStorageInner::BorrowedRef(b.clone_as_ref()),
            ValueStorageInner::Revalidating(f) => ValueStorageInner::Revalidating(f.clone()),
            ValueStorageInner::Source(source) => ValueStorageInner::Source(source.clone()),
        };
        Self { inner }
    }
}

impl<T: Copy> PartialEq for ValueStorage<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        if let (Ok(left), Ok(right)) = (self.as_ref(), other.as_ref()) {
            return *left == *right;
        }

        match (&self.inner, &other.inner) {
            (ValueStorageInner::Owned(a), ValueStorageInner::Owned(b)) => a == b,
            (ValueStorageInner::OwnedReadOnly(a), ValueStorageInner::OwnedReadOnly(b)) => a == b,
            (ValueStorageInner::BorrowedRef(a), ValueStorageInner::BorrowedRef(b)) => {
                a.as_ptr() == b.as_ptr()
            }
            (ValueStorageInner::BorrowedMut(a), ValueStorageInner::BorrowedMut(b)) => {
                a.as_ptr() == b.as_ptr()
            }
            (ValueStorageInner::Revalidating(a), ValueStorageInner::Revalidating(b)) => {
                a.same_identity(b)
            }
            (ValueStorageInner::Source(a), ValueStorageInner::Source(b)) => a.same_identity(b),
            _ => false,
        }
    }
}

impl<T: Copy> BorrowableStorage<T> for ValueStorage<T> {
    unsafe fn borrowed_ref(ptr: *const T, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ValueStorageInner::BorrowedRef(unsafe { BorrowedRef::new(ptr, validity) }),
        }
    }

    unsafe fn borrowed_mut(ptr: *mut T, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ValueStorageInner::BorrowedMut(unsafe { BorrowedMut::new(ptr, validity) }),
        }
    }

    unsafe fn borrowed_mut_tracked(
        ptr: *mut T,
        validity: ValidityFlag,
        context: ComponentWriteContext,
    ) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged.
            inner: ValueStorageInner::BorrowedMut(unsafe {
                BorrowedMut::new_tracked(ptr, validity, context)
            }),
        }
    }

    fn snapshot(value: &T) -> Self {
        Self {
            inner: ValueStorageInner::OwnedReadOnly(*value),
        }
    }

    unsafe fn revalidating_field(
        world_ptr: *mut World,
        entity: Entity,
        component_id: ComponentId,
        offset: usize,
        validity: ValidityFlagWithMode,
    ) -> Self {
        // SAFETY: forwards this constructor's contract unchanged
        unsafe { Self::revalidating(world_ptr, entity, component_id, offset, validity) }
    }

    fn revalidating_source(source: RevalidatingSource<T>) -> Self
    where
        T: 'static,
    {
        Self {
            inner: ValueStorageInner::Source(Box::new(source)),
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

    /// Create an enforced read-only snapshot.
    pub const fn read_only_snapshot(value: T) -> Self {
        Self {
            inner: ValueStorageInner::OwnedReadOnly(value),
        }
    }

    /// Create borrowed value storage, choosing read vs write from the transport mode.
    ///
    /// This is the bridge boundary constructor: `ValidityFlagWithMode` still carries
    /// the mode across the FFI layer, and is resolved here into a typed borrowed
    /// variant that no longer stores the mode.
    ///
    /// # Safety
    /// - `ptr` must point to valid `T` for as long as `validity` is non-Invalid
    /// - `ptr` must be safe to write through when `validity.access_mode()` is `Write`
    pub unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self {
        let component_write = validity.component_write_context();
        match validity.access_mode() {
            // SAFETY: mode Write means ptr was obtained from a mutable borrow.
            AccessMode::Write => match component_write {
                // SAFETY: forwards this constructor's pointer and context contract.
                Some(context) => unsafe {
                    <Self as BorrowableStorage<T>>::borrowed_mut_tracked(
                        ptr,
                        validity.flag,
                        context,
                    )
                },
                // SAFETY: forwards this constructor's pointer contract.
                None => unsafe { <Self as BorrowableStorage<T>>::borrowed_mut(ptr, validity.flag) },
            },
            // SAFETY: read-only view of the same pointer
            _ => unsafe {
                <Self as BorrowableStorage<T>>::borrowed_ref(ptr as *const T, validity.flag)
            },
        }
    }

    /// Create a re-resolving field handle for a field of a wrapper component.
    ///
    /// Unlike `borrowed`, this caches no pointer: each access re-derives the field's
    /// current address from `(world_ptr, entity, component_id, offset)`, so it stays
    /// valid across structural mutations that move the component, and errors once the
    /// entity is despawned. Used for Vec3/Vec2 fields escaped from a `world.get`/
    /// `world.get_mut` proxy, where a cached borrow would dangle.
    ///
    /// # Safety
    /// - `world_ptr` must be valid while `validity` is active.
    /// - `entity`, `component_id` and `offset` must identify a field of type `T` at
    ///   `offset` bytes into that component's storage.
    pub unsafe fn revalidating(
        world_ptr: *mut World,
        entity: Entity,
        component_id: ComponentId,
        offset: usize,
        validity: ValidityFlagWithMode,
    ) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ValueStorageInner::Revalidating(Box::new(unsafe {
                RevalidatingField::new(world_ptr, entity, component_id, offset, validity)
            })),
        }
    }

    /// Get immutable reference to the value, checking validity
    ///
    /// # Errors
    /// Returns `StorageError::InvalidAccess` if the borrowed reference is no longer valid
    /// (i.e., accessed outside of system execution context)
    #[inline(always)]
    pub fn as_ref(&self) -> Result<StorageRef<'_, T>, StorageError> {
        match &self.inner {
            ValueStorageInner::Owned(value) | ValueStorageInner::OwnedReadOnly(value) => {
                Ok(StorageRef::Direct(value))
            }
            ValueStorageInner::BorrowedRef(b) => b.get().map(StorageRef::Direct),
            ValueStorageInner::BorrowedMut(b) => b.get().map(StorageRef::Direct),
            ValueStorageInner::Revalidating(f) => f.get::<T>().map(StorageRef::Direct),
            ValueStorageInner::Source(source) => source.resolve_ref().map(StorageRef::Source),
        }
    }

    /// Get mutable reference to the value, checking validity and write permission
    ///
    /// # Errors
    /// Returns `StorageError` if:
    /// - The borrowed reference is no longer valid
    /// - The value was borrowed immutably (`BorrowedRef`) but mutable access is attempted
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<StorageMut<'_, T>, StorageError> {
        match &mut self.inner {
            ValueStorageInner::Owned(value) => Ok(StorageMut::Direct(value)),
            ValueStorageInner::OwnedReadOnly(_) => Err(StorageError::OwnedFieldReadOnly),
            ValueStorageInner::BorrowedRef(_) => Err(StorageError::ReadOnly),
            ValueStorageInner::BorrowedMut(b) => b.get_mut().map(StorageMut::Direct),
            ValueStorageInner::Revalidating(f) => f.get_mut::<T>().map(StorageMut::Direct),
            ValueStorageInner::Source(source) => source.resolve_mut().map(StorageMut::Source),
        }
    }

    /// Check if this storage contains an owned value (including read-only snapshots)
    #[cfg(test)]
    pub fn is_owned(&self) -> bool {
        matches!(
            self.inner,
            ValueStorageInner::Owned(_) | ValueStorageInner::OwnedReadOnly(_)
        )
    }

    /// Check if this storage contains a borrowed value
    #[cfg(test)]
    pub fn is_borrowed(&self) -> bool {
        matches!(
            self.inner,
            ValueStorageInner::BorrowedRef(_)
                | ValueStorageInner::BorrowedMut(_)
                | ValueStorageInner::Revalidating(_)
                | ValueStorageInner::Source(_)
        )
    }

    /// Check if this storage is a read-only snapshot
    #[cfg(test)]
    pub fn is_owned_read_only(&self) -> bool {
        matches!(self.inner, ValueStorageInner::OwnedReadOnly(_))
    }

    /// Get the current value (returns a copy)
    ///
    /// For owned values, returns a copy.
    /// For borrowed values, copies the current value.
    ///
    /// # Errors
    /// Returns an error if the borrowed value is no longer valid.
    #[inline(always)]
    pub fn get(&self) -> Result<T, StorageError> {
        Ok(*self.as_ref()?)
    }

    /// Helper to borrow a field from the value storage
    ///
    /// This reduces boilerplate in field getters by unifying the owned/borrowed cases.
    /// Similar to ComponentStorage::borrow_field, but works with ValueStorage.
    ///
    /// Owned storage (including read-only snapshots) returns a read-only
    /// snapshot of the field. Borrowed storage returns a validity-bound field
    /// inheriting the parent's read/write access.
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
            ValueStorageInner::BorrowedRef(b) => b.borrow_field(field_accessor),
            ValueStorageInner::BorrowedMut(b) => b.borrow_field(field_accessor),
            // A sub-field of a re-resolving handle re-resolves too (composing offsets),
            // so it stays valid across the same structural moves rather than dangling.
            ValueStorageInner::Revalidating(f) => f.child_of::<T, F, S>(field_accessor),
            ValueStorageInner::Source(source) => {
                let resolved = source.resolve_ref()?;
                Ok(S::snapshot(field_accessor(&resolved)))
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

    /// Materialize an explicit read-only snapshot of a value field.
    pub fn snapshot_field_as<F: Clone, S, W>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        let value = self.as_ref()?;
        Ok(W::from_borrowed(S::snapshot(field_accessor(&value))))
    }

    /// Borrow a field through a paired path when this value came from an asset.
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
            ValueStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current);
                drop(current);
                S::revalidating_source(source.field(read, write))
            }
            _ => return self.borrow_field_as(read),
        };
        Ok(W::from_borrowed(storage))
    }

    /// Resolve an enum payload, validating the expected variant on every asset access.
    pub fn borrow_resolved_variant_as<F: Copy + 'static, W>(
        &self,
        name: &'static str,
        read: ReadVariant<T, F>,
        write: WriteVariant<T, F>,
    ) -> Result<W, StorageError>
    where
        T: 'static,
        W: FromBorrowedStorage<ValueStorage<F>>,
    {
        let storage = match &self.inner {
            ValueStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current).ok_or(StorageError::VariantChanged(name))?;
                drop(current);
                ValueStorage::revalidating_source(source.variant(name, read, write))
            }
            _ => {
                let current = self.as_ref()?;
                let value = read(&current).ok_or(StorageError::VariantChanged(name))?;
                ValueStorage::snapshot(value)
            }
        };
        Ok(W::from_borrowed(storage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validity_guard::{AccessMode, ValidityFlag, ValidityGuard};

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct TestValue {
        x: f32,
        y: f32,
    }

    #[test]
    fn test_owned_storage() {
        let storage = ValueStorage::owned(TestValue { x: 1.0, y: 2.0 });
        assert!(storage.is_owned());
        assert!(!storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().x, 1.0);
    }

    #[test]
    fn test_owned_mutation() {
        let mut storage = ValueStorage::owned(TestValue { x: 1.0, y: 2.0 });
        storage.as_mut().unwrap().x = 42.0;
        assert_eq!(storage.as_ref().unwrap().x, 42.0);
    }

    #[test]
    fn test_borrowed_storage() {
        let mut value = TestValue { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage =
            unsafe { ValueStorage::borrowed(&mut value as *mut TestValue, validity.clone()) };

        assert!(!storage.is_owned());
        assert!(storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().x, 1.0);
    }

    #[test]
    fn test_borrowed_mutation() {
        let mut value = TestValue { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);

        let mut storage =
            unsafe { ValueStorage::borrowed(&mut value as *mut TestValue, validity.clone()) };

        storage.as_mut().unwrap().x = 42.0;
        assert_eq!(value.x, 42.0);
        assert_eq!(storage.as_ref().unwrap().x, 42.0);
    }

    #[test]
    fn test_validity_enforcement() {
        let mut value = TestValue { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage =
            unsafe { ValueStorage::borrowed(&mut value as *mut TestValue, validity.clone()) };

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
        let mut value = TestValue { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read); // Read-only borrow!

        let mut storage =
            unsafe { ValueStorage::borrowed(&mut value as *mut TestValue, validity.clone()) };

        // Read should work (with guard active)
        {
            let _guard = ValidityGuard::new(validity.flag.clone());
            assert!(storage.as_ref().is_ok());

            // Write should fail (borrowed as Read, not Write)
            assert!(storage.as_mut().is_err());
        }
    }

    #[test]
    fn test_get_owned() {
        let storage = ValueStorage::owned(TestValue { x: 1.0, y: 2.0 });
        let value = storage.get().unwrap();
        assert_eq!(value.x, 1.0);
        assert_eq!(value.y, 2.0);
    }

    #[test]
    fn test_get_borrowed() {
        let mut value = TestValue { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage = unsafe { ValueStorage::borrowed(&mut value as *mut TestValue, validity) };

        let copied = storage.get().unwrap();
        assert_eq!(copied.x, 1.0);
        assert_eq!(copied.y, 2.0);
    }

    #[test]
    fn test_snapshot_creates_owned_read_only() {
        let value = TestValue { x: 1.0, y: 2.0 };
        let storage = ValueStorage::snapshot(&value);
        assert!(storage.is_owned_read_only());
        assert!(storage.is_owned());
        assert!(!storage.is_borrowed());
    }

    #[test]
    fn test_owned_read_only_allows_reads() {
        let value = TestValue { x: 3.0, y: 4.0 };
        let storage = ValueStorage::snapshot(&value);
        assert_eq!(storage.as_ref().unwrap().x, 3.0);
        assert_eq!(storage.as_ref().unwrap().y, 4.0);
        assert_eq!(storage.get().unwrap(), value);
    }

    #[test]
    fn test_owned_read_only_rejects_writes() {
        let value = TestValue { x: 1.0, y: 2.0 };
        let mut storage = ValueStorage::snapshot(&value);
        assert!(matches!(
            storage.as_mut(),
            Err(StorageError::OwnedFieldReadOnly)
        ));
    }

    #[test]
    fn test_owned_read_only_is_independent_copy() {
        let value = TestValue { x: 1.0, y: 2.0 };
        let storage = ValueStorage::snapshot(&value);
        // Snapshot is a copy: doesn't alias the original
        assert_eq!(storage.as_ref().unwrap().x, 1.0);
        assert_eq!(storage.as_ref().unwrap().y, 2.0);
    }

    #[test]
    fn test_owned_read_only_clone() {
        let value = TestValue { x: 5.0, y: 6.0 };
        let storage = ValueStorage::snapshot(&value);
        let cloned = storage.clone();
        assert!(cloned.is_owned_read_only());
        assert_eq!(cloned.as_ref().unwrap().x, 5.0);
    }

    #[test]
    fn equality_compares_values_across_storage_modes() {
        let value = TestValue { x: 5.0, y: 6.0 };
        let owned = ValueStorage::owned(value);
        let snapshot = ValueStorage::snapshot(&value);

        assert_eq!(owned, snapshot);
    }

    #[test]
    fn test_owned_read_only_borrow_field_returns_snapshot() {
        let value = TestValue { x: 7.0, y: 8.0 };
        let storage = ValueStorage::snapshot(&value);
        let field: ValueStorage<f32> = storage.borrow_field(|v| &v.x).unwrap();
        assert!(field.is_owned_read_only());
        assert_eq!(field.get().unwrap(), 7.0);
        // Sub-field is also read-only
        let mut field = field;
        assert!(matches!(
            field.as_mut(),
            Err(StorageError::OwnedFieldReadOnly)
        ));
    }

    #[test]
    fn test_owned_borrow_field_returns_snapshot() {
        // Even plain Owned (not OwnedReadOnly) returns snapshot sub-fields
        let storage = ValueStorage::owned(TestValue { x: 10.0, y: 20.0 });
        let field: ValueStorage<f32> = storage.borrow_field(|v| &v.y).unwrap();
        assert!(field.is_owned_read_only());
        assert_eq!(field.get().unwrap(), 20.0);
    }

    #[test]
    fn test_mutably_borrowed_field_stays_live() {
        let mut value = TestValue { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let storage =
            unsafe { ValueStorage::borrowed(&mut value as *mut TestValue, validity.clone()) };
        let field: ValueStorage<f32> = storage.borrow_field(|v| &v.x).unwrap();
        assert!(field.is_borrowed());
    }

    #[test]
    fn test_read_only_borrowed_field_stays_validity_bound() {
        let value = TestValue { x: 1.0, y: 2.0 };
        let validity = ValidityFlag::new_read();
        let storage =
            unsafe { ValueStorage::borrowed_ref(&value as *const TestValue, validity.clone()) };

        let mut field: ValueStorage<f32> = storage.borrow_field(|v| &v.x).unwrap();
        assert!(field.is_borrowed());
        assert!(matches!(field.as_mut(), Err(StorageError::ReadOnly)));
        validity.set_invalid();
        assert!(matches!(field.get(), Err(StorageError::InvalidAccess)));
    }

    #[derive(bevy::ecs::component::Component)]
    #[repr(C, align(8))]
    struct Holder {
        v: [f32; 3],
    }

    #[derive(bevy::ecs::component::Component)]
    struct Tag;

    #[test]
    fn test_revalidating_reads_and_writes_live_component() {
        let mut world = World::new();
        let cid = world.register_component::<Holder>();
        let e = world.spawn(Holder { v: [1.0, 2.0, 3.0] }).id();
        let world_ptr: *mut World = &mut world;

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        // Handle to v[1]: offset = one f32 = 4 bytes.
        let mut storage: ValueStorage<f32> =
            unsafe { ValueStorage::revalidating(world_ptr, e, cid, 4, validity) };

        assert_eq!(storage.get().unwrap(), 2.0);
        *storage.as_mut().unwrap() = 9.0;
        // Write went through to the live component.
        assert_eq!(world.entity(e).get::<Holder>().unwrap().v[1], 9.0);
    }

    #[test]
    fn test_revalidating_tracks_after_archetype_move() {
        let mut world = World::new();
        let cid = world.register_component::<Holder>();
        let e = world.spawn(Holder { v: [1.0, 2.0, 3.0] }).id();
        let world_ptr: *mut World = &mut world;

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let mut storage: ValueStorage<f32> =
            unsafe { ValueStorage::revalidating(world_ptr, e, cid, 0, validity) };

        // Insert a second component: moves the entity to a new archetype/table, which
        // would dangle a cached pointer.
        world.entity_mut(e).insert(Tag);

        assert_eq!(storage.get().unwrap(), 1.0);
        *storage.as_mut().unwrap() = 7.0;
        assert_eq!(world.entity(e).get::<Holder>().unwrap().v[0], 7.0);
    }

    #[test]
    fn test_revalidating_errors_after_despawn() {
        let mut world = World::new();
        let cid = world.register_component::<Holder>();
        let e = world.spawn(Holder { v: [1.0, 2.0, 3.0] }).id();
        let world_ptr: *mut World = &mut world;

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let mut storage: ValueStorage<f32> =
            unsafe { ValueStorage::revalidating(world_ptr, e, cid, 0, validity) };

        assert!(storage.as_ref().is_ok());
        world.despawn(e);
        assert!(storage.as_ref().is_err());
        assert!(storage.as_mut().is_err());
        assert!(storage.get().is_err());
    }

    #[test]
    fn test_revalidating_read_mode_rejects_write() {
        let mut world = World::new();
        let cid = world.register_component::<Holder>();
        let e = world.spawn(Holder { v: [1.0, 2.0, 3.0] }).id();
        let world_ptr: *mut World = &mut world;

        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);
        let mut storage: ValueStorage<f32> =
            unsafe { ValueStorage::revalidating(world_ptr, e, cid, 0, validity) };

        assert!(storage.as_ref().is_ok());
        assert!(matches!(storage.as_mut(), Err(StorageError::ReadOnly)));
    }
}
