//! Core storage traits for PyBevy
//!
//! These traits define the interface for storage types that support borrowed references
//! with validity tracking.

use bevy::ecs::{component::ComponentId, entity::Entity, world::World};

use crate::{
    RevalidatingSource,
    component_change::ComponentWriteContext,
    validity_guard::{ValidityFlag, ValidityFlagWithMode},
};

/// Mark a getter result as an independently owned computed value.
#[inline(always)]
pub fn computed_owned<T>(value: T) -> T {
    value
}

/// Trait for storage types that can borrow field pointers with validity tracking
///
/// This is implemented by ValueStorage and FieldStorage to provide a
/// unified interface for creating borrowed field references. Read vs write access is
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

    /// Create mutable storage that marks its owning ECS component only on write.
    ///
    /// # Safety
    /// The pointer and validity requirements of [`Self::borrowed_mut`] apply, and
    /// `context` must identify the component allocation containing `ptr`.
    unsafe fn borrowed_mut_tracked(
        ptr: *mut T,
        validity: ValidityFlag,
        context: ComponentWriteContext,
    ) -> Self;

    /// Create a read-only owned snapshot (copy) of the given value.
    ///
    /// Used when extracting fields from owned/temporary storage.
    /// The returned storage allows reads but errors on writes with
    /// `StorageError::OwnedFieldReadOnly`.
    fn snapshot(value: &T) -> Self
    where
        T: Clone;

    /// Create a re-resolving field handle keyed by ECS identity rather than a cached
    /// pointer.
    ///
    /// Each access re-derives the field's address from
    /// `(world_ptr, entity, component_id, offset)`, so the handle survives structural
    /// mutations that relocate the component and errors after the entity is despawned.
    /// Used for fields that escape a long-lived `world.get`/`world.get_mut` handle,
    /// where a cached borrow would dangle. `validity` still carries the read/write mode.
    ///
    /// # Safety
    /// - `world_ptr` must be valid while `validity` is non-Invalid
    /// - `(entity, component_id, offset)` must identify a live field of type `T`
    unsafe fn revalidating_field(
        world_ptr: *mut World,
        entity: Entity,
        component_id: ComponentId,
        offset: usize,
        validity: ValidityFlagWithMode,
    ) -> Self;

    /// Create storage backed by a typed path into a live Bevy asset.
    fn revalidating_source(source: RevalidatingSource<T>) -> Self
    where
        T: 'static;
}

/// Trait for Python wrapper types that can be created from borrowed storage
///
/// This enables the `borrow_field_as` helper to return the final Python type directly:
/// `self.storage.borrow_field_as(|t| &t.translation)` instead of
/// `Ok(PyVec3::from_borrowed(self.storage.borrow_field(|t| &t.translation)?))`.
pub trait FromBorrowedStorage<S> {
    fn from_borrowed(storage: S) -> Self;
}
