//! Typed borrowed primitives shared by every storage type
//!
//! `BorrowedRef<T>` and `BorrowedMut<T>` encode read vs write access at the type
//! level instead of via a runtime `AccessMode` flag. Each carries a raw pointer
//! and a plain `ValidityFlag` (the "system still executing" gate). Native
//! component mutable borrows can additionally carry a write context so Bevy
//! change detection is triggered on the first actual write, not on extraction.
//!
//! The storage types (`ValueStorage`, `FieldStorage`, `ComponentStorage`,
//! `ResourceStorage`, `AssetStorage`) wrap these two types in
//! their borrowed variants, so the `Send`/`Sync` and `borrow_field` logic lives
//! here once rather than being duplicated per storage type.

use bevy::ecs::{component::ComponentId, entity::Entity, world::World};

use crate::{
    component_change::ComponentWriteContext,
    storage_error::StorageError,
    storage_traits::BorrowableStorage,
    validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode},
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
    // Kept inline to avoid a heap allocation for every mutable query row and
    // every nested tracked field wrapper.
    component_write: Option<ComponentWriteContext>,
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
    /// The validity flag gating this borrow (expires at system end).
    pub fn validity(&self) -> &ValidityFlag {
        &self.validity
    }

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
        Self {
            ptr,
            validity,
            component_write: None,
        }
    }

    /// Create a mutable borrow that marks its owning component on actual writes.
    ///
    /// # Safety
    /// The requirements of [`Self::new`] and [`ComponentWriteContext::new`] apply.
    #[inline(always)]
    pub unsafe fn new_tracked(
        ptr: *mut T,
        validity: ValidityFlag,
        component_write: ComponentWriteContext,
    ) -> Self {
        Self {
            ptr,
            validity,
            component_write: Some(component_write),
        }
    }

    #[inline(always)]
    pub fn get(&self) -> Result<&T, StorageError> {
        self.validity.check_read()?;
        let ptr = match self.component_write {
            Some(context) => context.resolve()? as *const T,
            None => self.ptr as *const T,
        };
        // SAFETY: validity checked above; ptr is either the original borrow or was
        // re-resolved from the same live component identity.
        Ok(unsafe { &*ptr })
    }

    #[inline(always)]
    pub fn get_mut(&mut self) -> Result<&mut T, StorageError> {
        self.validity.check_write()?;
        if let Some(context) = self.component_write {
            self.ptr = context.resolve_mut()? as *mut T;
        }
        // SAFETY: validity checked above; ptr came from the latest mutable component
        // access chain or from the original &mut chain per new()'s contract.
        Ok(unsafe { &mut *self.ptr })
    }

    /// Replace the stored pointer with one derived from a newer `&mut` chain.
    ///
    /// Deriving a fresh `&mut World` (as the asset change tracker does to queue
    /// `AssetEvent::Modified`) supersedes any pointer taken from an earlier
    /// chain. The caller must hand the resulting pointer back here so later
    /// writes go through the live chain instead of the invalidated one.
    ///
    /// # Safety
    /// - `ptr` must point to valid `T` for as long as `validity` is non-Invalid.
    /// - `ptr` must have been obtained from the most recent `&mut T` chain.
    #[inline(always)]
    pub unsafe fn adopt_ptr(&mut self, ptr: *mut T) {
        self.ptr = ptr;
    }

    /// Create a second mutable handle to the same data, sharing the validity flag.
    ///
    /// Used for intentional pointer sharing (e.g. `AnimationPlayer` handing a
    /// borrow to `ActiveAnimation`). Not exposed as `Clone` to keep accidental
    /// aliasing out of derive-generated code.
    #[inline(always)]
    pub fn share(&self) -> Self {
        Self {
            ptr: self.ptr,
            validity: self.validity.clone(),
            component_write: self.component_write,
        }
    }

    /// Downgrade to a read-only `BorrowedRef`, sharing the same validity flag.
    #[inline(always)]
    pub fn clone_as_ref(&self) -> BorrowedRef<T> {
        // SAFETY: read-only downgrade sharing the same ptr and flag
        unsafe { BorrowedRef::new(self.ptr as *const T, self.validity.clone()) }
    }

    #[inline(always)]
    /// The validity flag gating this borrow (expires at system end).
    pub fn validity(&self) -> &ValidityFlag {
        &self.validity
    }

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
        let base = match self.component_write {
            Some(context) => context.resolve()? as *mut T,
            None => self.ptr,
        };
        // SAFETY: validity checked above; base points to the current parent value.
        let offset =
            unsafe { (field_accessor(&*base) as *const F).byte_offset_from(base as *const u8) };
        // SAFETY: offset locates the selected field within the live parent value.
        let field_ptr = unsafe { (base as *mut u8).offset(offset) as *mut F };
        // SAFETY: the untracked arm inherits the parent's mutable provenance. In
        // the tracked arm, every mutable access re-resolves through the context's
        // mutable component access before dereferencing this cached pointer.
        Ok(unsafe {
            match self.component_write {
                Some(context) => S::borrowed_mut_tracked(
                    field_ptr,
                    self.validity.clone(),
                    context.child(offset as usize),
                ),
                None => S::borrowed_mut(field_ptr, self.validity.clone()),
            }
        })
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
        let base = match self.component_write {
            Some(context) => context.resolve()? as *mut T,
            None => self.ptr,
        };
        // SAFETY: validity was checked and base points to the current parent value.
        let parent = unsafe { &*base };
        let offset = match field_accessor(parent) {
            // SAFETY: field_ref points inside *base.
            Some(field_ref) => unsafe {
                (field_ref as *const F).byte_offset_from(base as *const u8)
            },
            None => return Ok(None),
        };
        // SAFETY: offset locates the selected field within the live parent value.
        let field_ptr = unsafe { (base as *mut u8).offset(offset) as *mut F };
        // SAFETY: the untracked arm inherits the parent's mutable provenance. In
        // the tracked arm, every mutable access re-resolves through the context's
        // mutable component access before dereferencing this cached pointer.
        Ok(Some(unsafe {
            match self.component_write {
                Some(context) => S::borrowed_mut_tracked(
                    field_ptr,
                    self.validity.clone(),
                    context.child(offset as usize),
                ),
                None => S::borrowed_mut(field_ptr, self.validity.clone()),
            }
        }))
    }
}

/// Re-derive the address of a component field: the component's current base pointer
/// (from an immutable `get_by_id`, matching how construction resolves it) plus the
/// field's byte offset. `None` if the entity was despawned or the component removed.
///
/// # Safety
/// `world_ptr` must be valid and free of a competing mutable borrow for the call.
#[inline]
pub(crate) unsafe fn revalidate_field_ptr(
    world_ptr: *mut World,
    entity: Entity,
    component_id: ComponentId,
    offset: usize,
) -> Option<*mut u8> {
    // SAFETY: caller guarantees `world_ptr` is valid for a shared borrow here
    let world = unsafe { &*world_ptr };
    let entity_ref = world.get_entity(entity).ok()?;
    let base = entity_ref.get_by_id(component_id).ok()?.as_ptr();
    // SAFETY: `offset` is within the component's layout per the caller's contract
    Some(unsafe { base.add(offset) })
}

/// Re-derive the address of a writable component field and mark the component changed.
///
/// # Safety
/// `world_ptr` must be valid and have exclusive access for the call.
#[inline]
pub(crate) unsafe fn revalidate_field_ptr_mut(
    world_ptr: *mut World,
    entity: Entity,
    component_id: ComponentId,
    offset: usize,
) -> Option<*mut u8> {
    // SAFETY: caller guarantees exclusive access to the live World.
    let world = unsafe { &mut *world_ptr };
    let value = world.get_mut_by_id(entity, component_id)?;
    // `into_inner` marks the component changed, matching Bevy's typed `Mut<T>`.
    let base = value.into_inner().as_ptr();
    // SAFETY: `offset` is within the component's layout per the caller's contract.
    Some(unsafe { base.add(offset) })
}

/// ECS identity of a component field, re-resolved on each access instead of cached.
///
/// Shared by every storage type's `Revalidating` variant. Because it holds no pointer,
/// a structural mutation that relocates the component does not dangle it; access after
/// the entity is despawned errors with [`StorageError::EntityUnavailable`]. `offset` is
/// the field's byte offset within its component (0 for a whole-component handle).
///
/// Public (like `BorrowedRef`/`BorrowedMut`) only because it appears in each storage's
/// `pub` `Revalidating` variant; all fields stay private.
#[derive(Debug, Clone)]
pub struct RevalidatingField {
    world_ptr: *mut World,
    entity: Entity,
    component_id: ComponentId,
    offset: usize,
    /// Validity + read/write mode. `check_write` still enforces read-only access, so a
    /// handle from `world.get` rejects mutation, same as `BorrowedRef`.
    validity: ValidityFlagWithMode,
}

// SAFETY: the `*mut World` is just an address; it is only dereferenced through a fresh
// re-resolve gated by `validity` (Arc<AtomicU8>, itself Send + Sync), which is
// invalidated when the owning system exits. The other fields are plain Copy data.
unsafe impl Send for RevalidatingField {}
// SAFETY: same argument as the impl above
unsafe impl Sync for RevalidatingField {}

impl RevalidatingField {
    /// # Safety
    /// - `world_ptr` must be valid while `validity` is non-Invalid.
    /// - `(entity, component_id, offset)` must identify a live field of the type the
    ///   caller will read/write through this handle.
    #[inline]
    pub(crate) unsafe fn new(
        world_ptr: *mut World,
        entity: Entity,
        component_id: ComponentId,
        offset: usize,
        validity: ValidityFlagWithMode,
    ) -> Self {
        Self {
            world_ptr,
            entity,
            component_id,
            offset,
            validity,
        }
    }

    #[inline(always)]
    pub(crate) fn validity(&self) -> &ValidityFlag {
        &self.validity.flag
    }

    pub(crate) fn clone_as_ref(&self) -> Self {
        Self {
            world_ptr: self.world_ptr,
            entity: self.entity,
            component_id: self.component_id,
            offset: self.offset,
            validity: self.validity.flag.with_access_mode(AccessMode::Read),
        }
    }

    #[inline]
    fn resolve(&self) -> Result<*mut u8, StorageError> {
        // SAFETY: forwarded from this handle's construction contract
        unsafe { revalidate_field_ptr(self.world_ptr, self.entity, self.component_id, self.offset) }
            .ok_or(StorageError::EntityUnavailable)
    }

    #[inline]
    fn resolve_mut(&mut self) -> Result<*mut u8, StorageError> {
        // SAFETY: forwarded from this handle's construction contract; `&mut self`
        // represents the handle's exclusive write access.
        unsafe {
            revalidate_field_ptr_mut(self.world_ptr, self.entity, self.component_id, self.offset)
        }
        .ok_or(StorageError::EntityUnavailable)
    }

    /// Read the field as `&T`, checking validity. `T` must be the field's type.
    #[inline]
    pub(crate) fn get<T>(&self) -> Result<&T, StorageError> {
        self.validity.check_read()?;
        let ptr = self.resolve()?;
        // SAFETY: validity checked; ptr re-resolved to the live field of type T
        Ok(unsafe { &*(ptr as *const T) })
    }

    /// Write the field as `&mut T`, checking validity and write permission.
    ///
    /// Takes `&mut self` (like `BorrowedMut::get_mut`) so the returned `&mut T` is not a
    /// mutable borrow synthesized from a shared one; every caller reaches this from a
    /// storage `as_mut(&mut self)`.
    #[inline]
    pub(crate) fn get_mut<T>(&mut self) -> Result<&mut T, StorageError> {
        self.validity.check_write()?;
        let ptr = self.resolve_mut()?;
        // SAFETY: validity + write permission checked; ptr re-resolved to the live field
        Ok(unsafe { &mut *(ptr as *mut T) })
    }

    /// Produce a re-resolving handle for a sub-field, composing byte offsets so the
    /// child re-resolves from the same entity/component. `T` is this handle's value
    /// type, `F` the sub-field type, `S` the child storage.
    pub(crate) fn child_of<T, F, S>(
        &self,
        field_accessor: impl FnOnce(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        self.validity.check_read()?;
        let base = self.resolve()?;
        // SAFETY: validity checked; base re-resolved to the live value of type T
        let value_ref: &T = unsafe { &*(base as *const T) };
        let inner = field_accessor(value_ref) as *const F as usize;
        let child_offset = self.offset + inner.wrapping_sub(base as usize);
        // SAFETY: the sub-field lives at `child_offset` within the same component,
        // re-resolved per access exactly like this handle
        Ok(unsafe {
            S::revalidating_field(
                self.world_ptr,
                self.entity,
                self.component_id,
                child_offset,
                self.validity.clone(),
            )
        })
    }

    /// Like [`child_of`](Self::child_of) for an `Option<F>` sub-field: `Ok(None)` when
    /// the option is empty, else a re-resolving handle to the contained `F`.
    pub(crate) fn child_of_optional<T, F, S>(
        &self,
        field_accessor: impl FnOnce(&T) -> &Option<F>,
    ) -> Result<Option<S>, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        self.validity.check_read()?;
        let base = self.resolve()?;
        // SAFETY: validity checked; base re-resolved to the live value of type T
        let value_ref: &T = unsafe { &*(base as *const T) };
        match field_accessor(value_ref) {
            Some(field_ref) => {
                let inner = field_ref as *const F as usize;
                let child_offset = self.offset + inner.wrapping_sub(base as usize);
                // SAFETY: the contained field lives at `child_offset` within the component
                Ok(Some(unsafe {
                    S::revalidating_field(
                        self.world_ptr,
                        self.entity,
                        self.component_id,
                        child_offset,
                        self.validity.clone(),
                    )
                }))
            }
            None => Ok(None),
        }
    }

    /// Two handles are equal when they name the same field of the same entity.
    #[inline]
    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        self.entity == other.entity
            && self.component_id == other.component_id
            && self.offset == other.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_storage::ValueStorage;

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

    #[test]
    fn test_borrowed_ref_get_returns_value() {
        let value = 42u32;
        let flag = ValidityFlag::new_read();
        // SAFETY: value outlives the borrow within this test scope
        let borrow = unsafe { BorrowedRef::new(&value, flag) };
        assert_eq!(*borrow.get().unwrap(), 42);
    }

    #[test]
    fn test_borrowed_ref_get_rejects_when_invalid() {
        let value = 42u32;
        let flag = ValidityFlag::new();
        // SAFETY: value outlives the borrow within this test scope
        let borrow = unsafe { BorrowedRef::new(&value, flag) };
        assert!(borrow.get().is_err());
    }

    #[test]
    fn test_borrowed_ref_clone_shares_validity() {
        let value = 7u32;
        let flag = ValidityFlag::new_read();
        // SAFETY: value outlives the borrow within this test scope
        let borrow = unsafe { BorrowedRef::new(&value, flag.clone()) };
        let cloned = borrow.clone();
        assert_eq!(*cloned.get().unwrap(), 7);
        flag.set_invalid();
        assert!(cloned.get().is_err());
        assert!(borrow.get().is_err());
    }

    #[test]
    fn test_borrowed_ref_borrow_field_returns_borrowed_storage() {
        let pair = (1u32, 2u32);
        let flag = ValidityFlag::new_read();
        // SAFETY: pair outlives the borrow within this test scope
        let borrow = unsafe { BorrowedRef::new(&pair, flag) };
        let field: ValueStorage<u32> = borrow.borrow_field(|p| &p.1).unwrap();
        assert_eq!(field.get().unwrap(), 2);
    }

    #[test]
    fn test_borrowed_ref_borrow_optional_field_some() {
        let opt: Option<u32> = Some(99);
        let flag = ValidityFlag::new_read();
        // SAFETY: opt outlives the borrow within this test scope
        let borrow = unsafe { BorrowedRef::new(&opt, flag) };
        let field: Option<ValueStorage<u32>> = borrow.borrow_optional_field(|o| o).unwrap();
        assert!(field.is_some());
        assert_eq!(field.unwrap().get().unwrap(), 99);
    }

    #[test]
    fn test_borrowed_ref_borrow_optional_field_none() {
        let opt: Option<u32> = None;
        let flag = ValidityFlag::new_read();
        // SAFETY: opt outlives the borrow within this test scope
        let borrow = unsafe { BorrowedRef::new(&opt, flag) };
        let field: Option<ValueStorage<u32>> = borrow.borrow_optional_field(|o| o).unwrap();
        assert!(field.is_none());
    }

    #[test]
    fn test_borrowed_mut_get_reads_value() {
        let mut value = 7u32;
        let flag = ValidityFlag::new_write();
        // SAFETY: value outlives the borrow within this test scope
        let borrow = unsafe { BorrowedMut::new(&mut value, flag) };
        assert_eq!(*borrow.get().unwrap(), 7);
    }

    #[test]
    fn test_borrowed_mut_get_mut_modifies_value() {
        let mut value = 7u32;
        let flag = ValidityFlag::new_write();
        // SAFETY: value outlives the borrow within this test scope
        let mut borrow = unsafe { BorrowedMut::new(&mut value, flag) };
        *borrow.get_mut().unwrap() = 8;
        assert_eq!(value, 8);
    }

    #[test]
    fn test_borrowed_mut_share_aliases() {
        let mut value = 7u32;
        let flag = ValidityFlag::new_write();
        // SAFETY: value outlives the borrow within this test scope
        let borrow = unsafe { BorrowedMut::new(&mut value, flag) };
        let mut shared = borrow.share();
        *shared.get_mut().unwrap() = 42;
        assert_eq!(value, 42);
    }

    #[test]
    fn test_borrowed_mut_share_shares_validity() {
        let mut value = 7u32;
        let flag = ValidityFlag::new_write();
        // SAFETY: value outlives the borrow within this test scope
        let borrow = unsafe { BorrowedMut::new(&mut value, flag.clone()) };
        let mut shared = borrow.share();
        flag.set_invalid();
        assert!(shared.get_mut().is_err());
    }

    #[test]
    fn test_borrowed_mut_clone_as_ref_is_readonly() {
        let mut value = 7u32;
        let flag = ValidityFlag::new_write();
        // SAFETY: value outlives the borrow within this test scope
        let borrow = unsafe { BorrowedMut::new(&mut value, flag) };
        let reff = borrow.clone_as_ref();
        assert_eq!(*reff.get().unwrap(), 7);
    }

    #[test]
    fn test_borrowed_mut_borrow_field_returns_mutable_storage() {
        let mut pair = (1u32, 2u32);
        let flag = ValidityFlag::new_write();
        // SAFETY: pair outlives the borrow within this test scope
        let borrow = unsafe { BorrowedMut::new(&mut pair, flag) };
        let mut field: ValueStorage<u32> = borrow.borrow_field(|p| &p.1).unwrap();
        *field.as_mut().unwrap() = 99;
        assert_eq!(pair.1, 99);
    }

    #[test]
    fn test_borrowed_mut_borrow_optional_field_some() {
        let mut opt: Option<u32> = Some(5);
        let flag = ValidityFlag::new_write();
        // SAFETY: opt outlives the borrow within this test scope
        let borrow = unsafe { BorrowedMut::new(&mut opt, flag) };
        let field: Option<ValueStorage<u32>> = borrow.borrow_optional_field(|o| o).unwrap();
        assert!(field.is_some());
        let mut f = field.unwrap();
        *f.as_mut().unwrap() = 77;
        assert_eq!(opt, Some(77));
    }

    /// The field pointer is rebuilt from the base rather than cast from the
    /// accessor's reference, so the offset has to be applied in bytes. Applying
    /// it in units of `F` would scale it by `size_of::<F>()` and write outside
    /// the addressed field.
    #[test]
    fn test_borrowed_mut_borrow_field_offset_is_measured_in_bytes() {
        #[repr(C)]
        struct Mixed {
            head: u8,
            tail: u64,
        }

        let mut mixed = Mixed { head: 1, tail: 2 };
        let flag = ValidityFlag::new_write();
        // SAFETY: mixed outlives the borrow within this test scope
        let borrow = unsafe { BorrowedMut::new(&mut mixed, flag) };
        let mut field: ValueStorage<u64> = borrow.borrow_field(|m| &m.tail).unwrap();
        *field.as_mut().unwrap() = 99;
        assert_eq!(mixed.tail, 99);
        assert_eq!(mixed.head, 1, "write landed outside the addressed field");
    }

    #[test]
    fn test_borrowed_mut_borrow_optional_field_none() {
        let mut opt: Option<u32> = None;
        let flag = ValidityFlag::new_write();
        // SAFETY: opt outlives the borrow within this test scope
        let borrow = unsafe { BorrowedMut::new(&mut opt, flag) };
        let field: Option<ValueStorage<u32>> = borrow.borrow_optional_field(|o| o).unwrap();
        assert!(field.is_none());
    }
}
