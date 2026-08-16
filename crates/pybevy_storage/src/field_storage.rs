//! Generic field storage supporting both owned and borrowed instances
//!
//! This module provides storage for non-Copy field types (like TextureAtlas)
//! that can be accessed from component fields.
//!
//! Key difference from ValueStorage:
//! - FieldStorage is for non-Copy types (uses Box for owned values)
//! - ValueStorage is for Copy types (stores directly)
//!
//! # Safety Model
//!
//! FieldStorage follows the same safety model as ComponentStorage:
//!
//! ## Owned Mode
//!
//! - Data is stored in `Box<T>` with a `ValidityFlag` for field borrow tracking
//! - Field borrows share the parent's validity flag
//! - `Drop` invalidates the flag before the Box is dropped, preventing use-after-free
//!
//! ## Borrowed Mode
//!
//! - Raw pointer into parent storage (component or another FieldStorage)
//! - `ValidityFlagWithMode` tracks validity and read/write permission
//! - Inherits validity from parent (invalidated when parent's system exits)

use std::fmt::Debug;

use bevy::{
    asset::Asset,
    ecs::{component::ComponentId, entity::Entity, world::World},
};

use crate::{
    AssetStorage, ReadField, ReadIndex, ReadKey, ReadVariant, RevalidatingSource, StorageMut,
    StorageRef, WriteField, WriteIndex, WriteKey, WriteVariant,
    borrowed::{BorrowedMut, BorrowedRef, RevalidatingField},
    component_change::ComponentWriteContext,
    storage_error::StorageError,
    storage_traits::{BorrowableStorage, FromBorrowedStorage},
    validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode},
};

/// Generic storage for PyBevy field types (non-Copy types like TextureAtlas)
///
/// Supports two modes:
/// - `Owned`: Python-created value, stored in Box
/// - `Borrowed`: Reference to field in a component (e.g., sprite.texture_atlas)
///
/// # Type Parameters
/// - `T`: The field type (does not need to implement `Copy`)
///
/// # Safety
/// Borrowed variant contains a raw pointer to value data in a component.
/// The `ValidityFlagWithMode` ensures this pointer is only dereferenced during
/// system execution when the pointer is guaranteed to be valid.
#[derive(Debug)]
pub struct FieldStorage<T: Clone> {
    pub inner: FieldStorageInner<T>,
}

#[derive(Debug)]
pub enum FieldStorageInner<T: Clone> {
    /// Python-created value, stored in Box with validity tracking
    ///
    /// The ValidityFlag ensures that field borrows (raw pointers into the Box)
    /// cannot be used after the FieldStorage is dropped.
    Owned {
        /// Heap-allocated field data
        data: Box<T>,

        /// Validity tracking for field borrows
        /// Invalidated when FieldStorage is dropped
        validity: ValidityFlag,
    },

    /// Read-only snapshot of a field extracted from owned/temporary storage.
    /// Reads succeed; writes return `StorageError::OwnedFieldReadOnly`.
    OwnedReadOnly {
        /// Heap-allocated field data (clone of the original)
        data: Box<T>,
    },

    /// Read-only borrow into a component field
    BorrowedRef(BorrowedRef<T>),

    /// Mutable borrow into a component field
    BorrowedMut(BorrowedMut<T>),

    /// A non-Copy field reached from a long-lived `world.get`/`world.get_mut` handle.
    /// Caches no pointer: it re-derives the field's current address on every access, so
    /// it stays valid across structural moves and errors after despawn. Boxed so this
    /// rarely-used variant does not enlarge `FieldStorage`.
    Revalidating(Box<RevalidatingField>),

    /// A typed path into a live Bevy asset.
    Source(Box<RevalidatingSource<T>>),
}

impl<T: Clone> Clone for FieldStorage<T> {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            FieldStorageInner::Owned { data, validity: _ } => {
                // CRITICAL: Create a NEW validity flag for the clone.
                // Each owned instance needs independent validity tracking.
                FieldStorageInner::Owned {
                    data: Box::new((**data).clone()),
                    validity: ValidityFlag::new_write(),
                }
            }
            FieldStorageInner::OwnedReadOnly { data } => FieldStorageInner::OwnedReadOnly {
                data: Box::new((**data).clone()),
            },
            FieldStorageInner::BorrowedRef(b) => FieldStorageInner::BorrowedRef(b.clone()),
            // A cloned mutable borrow downgrades to read-only to avoid aliasing.
            FieldStorageInner::BorrowedMut(b) => FieldStorageInner::BorrowedRef(b.clone_as_ref()),
            FieldStorageInner::Revalidating(f) => FieldStorageInner::Revalidating(f.clone()),
            FieldStorageInner::Source(source) => FieldStorageInner::Source(source.clone()),
        };
        Self { inner }
    }
}

impl<T: Clone + PartialEq> PartialEq for FieldStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (
                FieldStorageInner::Owned { data: a, .. },
                FieldStorageInner::Owned { data: b, .. },
            ) => **a == **b,
            (
                FieldStorageInner::OwnedReadOnly { data: a },
                FieldStorageInner::OwnedReadOnly { data: b },
            ) => **a == **b,
            (FieldStorageInner::BorrowedRef(a), FieldStorageInner::BorrowedRef(b)) => {
                a.as_ptr() == b.as_ptr()
            }
            (FieldStorageInner::BorrowedMut(a), FieldStorageInner::BorrowedMut(b)) => {
                a.as_ptr() == b.as_ptr()
            }
            (FieldStorageInner::Revalidating(a), FieldStorageInner::Revalidating(b)) => {
                a.same_identity(b)
            }
            (FieldStorageInner::Source(a), FieldStorageInner::Source(b)) => a.same_identity(b),
            _ => false,
        }
    }
}

impl<T: Clone> Drop for FieldStorage<T> {
    fn drop(&mut self) {
        // Invalidate owned storage's validity flag before the Box is dropped.
        // This ensures any outstanding field borrows will fail their validity checks.
        //
        // For borrowed storage, the validity is managed by the parent (component or
        // another FieldStorage), so we don't invalidate here.
        if let FieldStorageInner::Owned { validity, .. } = &self.inner {
            validity.set_invalid();
        }
    }
}

impl<T: Clone> BorrowableStorage<T> for FieldStorage<T> {
    unsafe fn borrowed_ref(ptr: *const T, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: FieldStorageInner::BorrowedRef(unsafe { BorrowedRef::new(ptr, validity) }),
        }
    }

    unsafe fn borrowed_mut(ptr: *mut T, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: FieldStorageInner::BorrowedMut(unsafe { BorrowedMut::new(ptr, validity) }),
        }
    }

    unsafe fn borrowed_mut_tracked(
        ptr: *mut T,
        validity: ValidityFlag,
        context: ComponentWriteContext,
    ) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged.
            inner: FieldStorageInner::BorrowedMut(unsafe {
                BorrowedMut::new_tracked(ptr, validity, context)
            }),
        }
    }

    fn snapshot(value: &T) -> Self {
        Self {
            inner: FieldStorageInner::OwnedReadOnly {
                data: Box::new(value.clone()),
            },
        }
    }

    unsafe fn revalidating_field(
        world_ptr: *mut World,
        entity: Entity,
        component_id: ComponentId,
        offset: usize,
        validity: ValidityFlagWithMode,
    ) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: FieldStorageInner::Revalidating(Box::new(unsafe {
                RevalidatingField::new(world_ptr, entity, component_id, offset, validity)
            })),
        }
    }

    fn revalidating_source(source: RevalidatingSource<T>) -> Self
    where
        T: 'static,
    {
        Self {
            inner: FieldStorageInner::Source(Box::new(source)),
        }
    }
}

impl<T: Clone> FieldStorage<T> {
    /// Create owned field storage with validity tracking
    pub fn owned(value: T) -> Self {
        Self {
            inner: FieldStorageInner::Owned {
                data: Box::new(value),
                validity: ValidityFlag::new_write(),
            },
        }
    }

    /// Create borrowed field storage, choosing read vs write from the transport mode.
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

    /// Get immutable reference to the value, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<StorageRef<'_, T>, StorageError> {
        match &self.inner {
            FieldStorageInner::Owned { data, .. } | FieldStorageInner::OwnedReadOnly { data } => {
                Ok(StorageRef::Direct(&**data))
            }
            FieldStorageInner::BorrowedRef(b) => b.get().map(StorageRef::Direct),
            FieldStorageInner::BorrowedMut(b) => b.get().map(StorageRef::Direct),
            FieldStorageInner::Revalidating(f) => f.get::<T>().map(StorageRef::Direct),
            FieldStorageInner::Source(source) => source.resolve_ref().map(StorageRef::Source),
        }
    }

    /// Get mutable reference to the value, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<StorageMut<'_, T>, StorageError> {
        match &mut self.inner {
            FieldStorageInner::Owned { data, .. } => Ok(StorageMut::Direct(&mut **data)),
            FieldStorageInner::OwnedReadOnly { .. } => Err(StorageError::OwnedFieldReadOnly),
            FieldStorageInner::BorrowedRef(_) => Err(StorageError::ReadOnly),
            FieldStorageInner::BorrowedMut(b) => b.get_mut().map(StorageMut::Direct),
            FieldStorageInner::Revalidating(f) => f.get_mut::<T>().map(StorageMut::Direct),
            FieldStorageInner::Source(source) => source.resolve_mut().map(StorageMut::Source),
        }
    }

    /// Get the current value (returns a clone)
    #[inline(always)]
    pub fn get(&self) -> Result<T, StorageError> {
        Ok(self.as_ref()?.clone())
    }

    /// Borrow a field from the stored value
    ///
    /// Returns a borrowed reference to a nested field that can be mutated
    /// and have changes persist back to the original storage.
    ///
    /// # Example
    ///
    /// Prefer using `borrow_field_as` for simpler syntax:
    /// ```rust,ignore
    /// #[getter]
    /// pub fn physical_position(&self) -> PyResult<PyUVec2> {
    ///     self.storage.borrow_field_as(|v| &v.physical_position)
    /// }
    /// ```
    ///
    /// Or use `borrow_field` directly when more control is needed:
    /// ```rust,ignore
    /// let storage = self.storage.borrow_field(|v| &v.physical_position)?;
    /// Ok(PyUVec2::from_borrowed(storage))
    /// ```
    pub fn borrow_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        match &self.inner {
            FieldStorageInner::Owned { data, .. }
            | FieldStorageInner::OwnedReadOnly { data, .. } => {
                Ok(S::snapshot(field_accessor(&**data)))
            }
            FieldStorageInner::BorrowedRef(b) => b.borrow_field(field_accessor),
            FieldStorageInner::BorrowedMut(b) => b.borrow_field(field_accessor),
            FieldStorageInner::Revalidating(f) => f.child_of::<T, F, S>(field_accessor),
            FieldStorageInner::Source(source) => {
                let resolved = source.resolve_ref()?;
                Ok(S::snapshot(field_accessor(&resolved)))
            }
        }
    }

    /// Helper to borrow a field and wrap it in the final Python type
    ///
    /// Combines `borrow_field` with `FromBorrowedStorage::from_borrowed` to reduce boilerplate.
    ///
    /// # Example
    /// ```rust,ignore
    /// #[getter]
    /// pub fn physical_position(&self) -> PyResult<PyUVec2> {
    ///     self.storage.borrow_field_as(|v| &v.physical_position)
    /// }
    /// ```
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

    /// Materialize an explicit read-only snapshot of a field value.
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
            FieldStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current);
                drop(current);
                S::revalidating_source(source.field(read, write))
            }
            _ => return self.borrow_field_as(read),
        };
        Ok(W::from_borrowed(storage))
    }

    /// Resolve a non-Copy enum payload, validating its variant on every asset access.
    pub fn borrow_resolved_variant_as<F: Clone + 'static, W>(
        &self,
        name: &'static str,
        read: ReadVariant<T, F>,
        write: WriteVariant<T, F>,
    ) -> Result<W, StorageError>
    where
        T: 'static,
        W: FromBorrowedStorage<FieldStorage<F>>,
    {
        let storage = match &self.inner {
            FieldStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current).ok_or(StorageError::VariantChanged(name))?;
                drop(current);
                FieldStorage::revalidating_source(source.variant(name, read, write))
            }
            _ => {
                let current = self.as_ref()?;
                let value = read(&current).ok_or(StorageError::VariantChanged(name))?;
                FieldStorage::snapshot(value)
            }
        };
        Ok(W::from_borrowed(storage))
    }

    /// Resolve an indexed child, checking that the index still exists on every access.
    pub fn borrow_resolved_index_as<F: Clone + 'static, S, W>(
        &self,
        index: usize,
        read: ReadIndex<T, F>,
        write: WriteIndex<T, F>,
    ) -> Result<W, StorageError>
    where
        T: 'static,
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        let storage = match &self.inner {
            FieldStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current, index).ok_or(StorageError::IndexOutOfRange)?;
                drop(current);
                S::revalidating_source(source.index(index, read, write))
            }
            _ => {
                let current = self.as_ref()?;
                let value = read(&current, index).ok_or(StorageError::IndexOutOfRange)?;
                S::snapshot(value)
            }
        };
        Ok(W::from_borrowed(storage))
    }

    /// Resolve an indexed child whose Python wrapper uses asset storage.
    pub fn borrow_resolved_asset_index<F: Asset + Clone + 'static>(
        &self,
        index: usize,
        read: ReadIndex<T, F>,
        write: WriteIndex<T, F>,
    ) -> Result<AssetStorage<F>, StorageError>
    where
        T: 'static,
    {
        match &self.inner {
            FieldStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current, index).ok_or(StorageError::IndexOutOfRange)?;
                drop(current);
                Ok(AssetStorage::revalidating_source(
                    source.index(index, read, write),
                ))
            }
            _ => {
                let current = self.as_ref()?;
                let value = read(&current, index).ok_or(StorageError::IndexOutOfRange)?;
                Ok(AssetStorage::owned_readonly(value.clone()))
            }
        }
    }

    /// Resolve a keyed child, checking that the key still exists on every access.
    pub fn borrow_resolved_key_as<Key, F, S, W>(
        &self,
        key: Key,
        read: ReadKey<T, Key, F>,
        write: WriteKey<T, Key, F>,
    ) -> Result<W, StorageError>
    where
        T: 'static,
        Key: Clone + Debug + Eq + Send + Sync + 'static,
        F: Clone + 'static,
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        let storage = match &self.inner {
            FieldStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current, &key)
                    .ok_or_else(|| StorageError::KeyNotFound(format!("{key:?}")))?;
                drop(current);
                S::revalidating_source(source.key(key, read, write))
            }
            _ => {
                let current = self.as_ref()?;
                let value = read(&current, &key)
                    .ok_or_else(|| StorageError::KeyNotFound(format!("{key:?}")))?;
                S::snapshot(value)
            }
        };
        Ok(W::from_borrowed(storage))
    }

    /// Check if this storage contains an owned value (including read-only snapshots)
    #[cfg(test)]
    pub fn is_owned(&self) -> bool {
        matches!(
            self.inner,
            FieldStorageInner::Owned { .. } | FieldStorageInner::OwnedReadOnly { .. }
        )
    }

    #[cfg(test)]
    pub fn is_borrowed(&self) -> bool {
        matches!(
            self.inner,
            FieldStorageInner::BorrowedRef(_)
                | FieldStorageInner::BorrowedMut(_)
                | FieldStorageInner::Revalidating(_)
                | FieldStorageInner::Source(_)
        )
    }

    /// Check if this storage is a read-only snapshot
    #[cfg(test)]
    pub fn is_owned_read_only(&self) -> bool {
        matches!(self.inner, FieldStorageInner::OwnedReadOnly { .. })
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::change_detection::DetectChangesMut;

    use super::*;
    use crate::validity_guard::{AccessMode, ValidityFlag, ValidityGuard};

    #[derive(Clone, Debug, PartialEq)]
    struct TestField {
        name: String,
        values: Vec<i32>,
    }

    #[test]
    fn test_owned_storage() {
        let storage = FieldStorage::owned(TestField {
            name: "test".into(),
            values: vec![1, 2, 3],
        });
        assert!(storage.is_owned());
        assert!(!storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().name, "test");
        assert_eq!(storage.as_ref().unwrap().values, vec![1, 2, 3]);
    }

    #[test]
    fn test_owned_mutation() {
        let mut storage = FieldStorage::owned(TestField {
            name: "test".into(),
            values: vec![1, 2, 3],
        });
        storage.as_mut().unwrap().name = "modified".into();
        storage.as_mut().unwrap().values.push(4);
        assert_eq!(storage.as_ref().unwrap().name, "modified");
        assert_eq!(storage.as_ref().unwrap().values, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_borrowed_storage() {
        let mut field = TestField {
            name: "test".into(),
            values: vec![1, 2, 3],
        };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage =
            unsafe { FieldStorage::borrowed(&mut field as *mut TestField, validity.clone()) };

        assert!(!storage.is_owned());
        assert!(storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().name, "test");
    }

    #[test]
    fn test_borrowed_mutation_persists() {
        let mut field = TestField {
            name: "test".into(),
            values: vec![1, 2, 3],
        };
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);

        let mut storage =
            unsafe { FieldStorage::borrowed(&mut field as *mut TestField, validity.clone()) };

        storage.as_mut().unwrap().name = "modified".into();
        // Mutation persists through the raw pointer to the original
        assert_eq!(field.name, "modified");
        assert_eq!(storage.as_ref().unwrap().name, "modified");
    }

    #[test]
    fn test_validity_enforcement() {
        let mut field = TestField {
            name: "test".into(),
            values: vec![],
        };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage =
            unsafe { FieldStorage::borrowed(&mut field as *mut TestField, validity.clone()) };

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
        let mut field = TestField {
            name: "test".into(),
            values: vec![],
        };
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let mut storage =
            unsafe { FieldStorage::borrowed(&mut field as *mut TestField, validity.clone()) };

        {
            let _guard = ValidityGuard::new(validity.flag.clone());
            assert!(storage.as_ref().is_ok());
            // Write should fail (borrowed as Read, not Write)
            assert!(storage.as_mut().is_err());
        }
    }

    #[test]
    fn test_get_returns_clone() {
        let storage = FieldStorage::owned(TestField {
            name: "test".into(),
            values: vec![1, 2],
        });
        let cloned = storage.get().unwrap();
        assert_eq!(cloned.name, "test");
        assert_eq!(cloned.values, vec![1, 2]);
    }

    #[test]
    fn test_drop_invalidates_owned() {
        let validity_clone;
        {
            let storage = FieldStorage::owned(TestField {
                name: "test".into(),
                values: vec![],
            });
            // Get the validity flag before drop
            match &storage.inner {
                FieldStorageInner::Owned { validity, .. } => {
                    validity_clone = validity.clone();
                }
                _ => unreachable!(),
            }
            // Validity is write (owned is always valid)
            assert_eq!(validity_clone.get_mode(), AccessMode::Write);
        }
        // After drop, the validity flag should be Invalid
        assert_eq!(validity_clone.get_mode(), AccessMode::Invalid);
    }

    #[test]
    fn test_clone_owned_creates_independent_storage() {
        let mut storage = FieldStorage::owned(TestField {
            name: "original".into(),
            values: vec![1],
        });
        let mut cloned = storage.clone();

        // Mutating one shouldn't affect the other
        storage.as_mut().unwrap().name = "modified".into();
        assert_eq!(cloned.as_ref().unwrap().name, "original");

        cloned.as_mut().unwrap().values.push(2);
        assert_eq!(storage.as_ref().unwrap().values, vec![1]);
        assert_eq!(cloned.as_ref().unwrap().values, vec![1, 2]);
    }

    #[test]
    fn test_borrow_field_from_owned() {
        let storage = FieldStorage::owned(TestField {
            name: "test".into(),
            values: vec![10, 20, 30],
        });

        // Field from owned storage is a read-only snapshot
        let result: FieldStorage<Vec<i32>> = storage.borrow_field(|f| &f.values).unwrap();

        assert!(result.is_owned_read_only());
        assert_eq!(result.as_ref().unwrap(), &vec![10, 20, 30]);

        // Mutation is not allowed on read-only snapshots
        let mut result = result;
        assert!(matches!(
            result.as_mut(),
            Err(StorageError::OwnedFieldReadOnly)
        ));
    }

    #[test]
    fn test_borrow_field_from_borrowed() {
        let mut field = TestField {
            name: "test".into(),
            values: vec![10, 20],
        };
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);

        let storage =
            unsafe { FieldStorage::borrowed(&mut field as *mut TestField, validity.clone()) };

        let borrowed: FieldStorage<Vec<i32>> = storage.borrow_field(|f| &f.values).unwrap();

        assert!(borrowed.is_borrowed());
        assert_eq!(borrowed.as_ref().unwrap(), &vec![10, 20]);
    }

    #[test]
    fn test_borrow_field_mutation_persists_to_parent() {
        let mut field = TestField {
            name: "test".into(),
            values: vec![10, 20],
        };
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);

        let storage =
            unsafe { FieldStorage::borrowed(&mut field as *mut TestField, validity.clone()) };

        let mut borrowed: FieldStorage<Vec<i32>> = storage.borrow_field(|f| &f.values).unwrap();

        borrowed.as_mut().unwrap().push(30);
        // Mutation persists all the way to the original
        assert_eq!(field.values, vec![10, 20, 30]);
    }

    #[test]
    fn test_borrow_field_invalid_after_guard_dropped() {
        let mut field = TestField {
            name: "test".into(),
            values: vec![],
        };
        let flag = ValidityFlag::new();
        let validity = flag.with_access_mode(AccessMode::Write);

        let storage =
            unsafe { FieldStorage::borrowed(&mut field as *mut TestField, validity.clone()) };

        let borrowed: FieldStorage<Vec<i32>>;

        {
            let _guard = ValidityGuard::new(flag.clone());
            borrowed = storage.borrow_field(|f| &f.values).unwrap();
            assert!(borrowed.as_ref().is_ok());
        }

        // Guard dropped: borrowed field should also be invalid
        assert!(borrowed.as_ref().is_err());
    }

    #[test]
    fn test_revalidating_field_via_component_writes_through_and_tracks_move() {
        use crate::pycomponent::ComponentStorage;

        #[derive(bevy::ecs::component::Component)]
        #[repr(C)]
        struct StringHolder {
            text: String,
        }
        #[derive(bevy::ecs::component::Component)]
        struct Tag;

        let mut world = World::new();
        let cid = world.register_component::<StringHolder>();
        let e = world.spawn(StringHolder { text: "hi".into() }).id();
        let world_ptr: *mut World = &mut world;

        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);
        let comp =
            unsafe { ComponentStorage::<StringHolder>::revalidating(world_ptr, e, cid, validity) };
        // Non-Copy field reached from a re-resolving component: itself re-resolves.
        let mut field: FieldStorage<String> = comp.borrow_field(|c| &c.text).unwrap();
        assert!(field.is_borrowed());
        assert_eq!(field.as_ref().unwrap(), "hi");

        world.entity_mut(e).insert(Tag); // archetype move
        field.as_mut().unwrap().push_str("!");
        assert_eq!(world.entity(e).get::<StringHolder>().unwrap().text, "hi!");

        world.despawn(e);
        assert!(field.as_ref().is_err());
        assert!(field.as_mut().is_err());
    }

    #[test]
    fn test_borrowed_with_write_context_marks_component_changed() {
        #[derive(bevy::ecs::component::Component)]
        #[repr(C)]
        struct StringHolder {
            text: String,
        }

        let mut world = World::new();
        let component_id = world.register_component::<StringHolder>();
        let entity = world.spawn(StringHolder { text: "hi".into() }).id();
        let last_run = world.read_change_tick();
        world.increment_change_tick();
        world.increment_change_tick();
        let this_run = world.read_change_tick();

        let ptr = {
            let mut component = world.get_mut::<StringHolder>(entity).unwrap();
            &mut component.bypass_change_detection().text as *mut String
        };
        let world_cell = world.as_unsafe_world_cell();
        // SAFETY: the test keeps `world` alive and performs no competing component
        // access while the storage is used.
        let context = unsafe {
            crate::ComponentWriteContext::new_with_offset(
                world_cell,
                entity,
                component_id,
                0,
                last_run,
                this_run,
            )
        };
        let validity = ValidityFlag::new_write()
            .with_access_mode(AccessMode::Write)
            .with_component_write_context(context);
        // SAFETY: ptr and context identify the same live field and the validity
        // flag remains active for the test.
        let mut storage: FieldStorage<String> = unsafe { FieldStorage::borrowed(ptr, validity) };

        assert_eq!(storage.as_ref().unwrap(), "hi");
        let ticks = world
            .entity(entity)
            .get_change_ticks_by_id(component_id)
            .unwrap();
        assert!(!ticks.is_changed(last_run, this_run));

        storage.as_mut().unwrap().push('!');
        assert_eq!(
            world.entity(entity).get::<StringHolder>().unwrap().text,
            "hi!"
        );
        let ticks = world
            .entity(entity)
            .get_change_ticks_by_id(component_id)
            .unwrap();
        assert!(ticks.is_changed(last_run, this_run));
    }

    #[test]
    fn test_revalidating_field_read_mode_rejects_write() {
        let mut world = World::new();

        #[derive(bevy::ecs::component::Component)]
        #[repr(C)]
        struct StringHolder {
            text: String,
        }

        let cid = world.register_component::<StringHolder>();
        let e = world.spawn(StringHolder { text: "hi".into() }).id();
        let world_ptr: *mut World = &mut world;

        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);
        // text is the first field of a #[repr(C)] struct, so offset 0.
        let mut storage: FieldStorage<String> = unsafe {
            <FieldStorage<String> as BorrowableStorage<String>>::revalidating_field(
                world_ptr, e, cid, 0, validity,
            )
        };
        assert!(storage.as_ref().is_ok());
        assert!(matches!(storage.as_mut(), Err(StorageError::ReadOnly)));
    }
}
