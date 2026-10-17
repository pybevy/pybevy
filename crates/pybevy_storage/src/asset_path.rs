//! Typed, re-resolving paths into borrowed Bevy values and assets.

use std::{
    any::{Any, TypeId, type_name},
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
    },
};

use bevy::{
    asset::{Asset, UntypedAssetId},
    ecs::component::Component,
};
use smallvec::SmallVec;

use crate::{
    AssetResourceReadGuard, AssetResourceWriteGuard, AssetStorage, ComponentStorage,
    ComponentStorageInner, FieldStorage, FieldStorageInner, StorageError, ValueStorage,
    borrowed::{BorrowedMut, BorrowedRef, RevalidatingField},
    pyasset::AssetStorageInner,
    value_storage::ValueStorageInner,
};

pub type ReadField<Parent, Field> = for<'a> fn(&'a Parent) -> &'a Field;
pub type WriteField<Parent, Field> = for<'a> fn(&'a mut Parent) -> &'a mut Field;
pub type ReadVariant<Parent, Field> = for<'a> fn(&'a Parent) -> Option<&'a Field>;
pub type WriteVariant<Parent, Field> = for<'a> fn(&'a mut Parent) -> Option<&'a mut Field>;
pub type ReadIndex<Parent, Field> = for<'a> fn(&'a Parent, usize) -> Option<&'a Field>;
pub type WriteIndex<Parent, Field> = for<'a> fn(&'a mut Parent, usize) -> Option<&'a mut Field>;
pub type ReadKey<Parent, Key, Field> = for<'a> fn(&'a Parent, &Key) -> Option<&'a Field>;
pub type WriteKey<Parent, Key, Field> = for<'a> fn(&'a mut Parent, &Key) -> Option<&'a mut Field>;
pub type ValidateField<Field> = fn(&Field) -> Result<(), StorageError>;

trait ErasedPathStep: fmt::Debug + Send + Sync {
    fn input_type(&self) -> TypeId;
    fn output_type(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
    fn same_identity(&self, other: &dyn ErasedPathStep) -> bool;

    /// # Safety
    /// `parent` must point to a live value of [`Self::input_type`].
    unsafe fn project_ref(&self, parent: *const u8) -> Result<*const u8, StorageError>;

    /// # Safety
    /// `parent` must uniquely point to a live value of [`Self::input_type`].
    unsafe fn project_mut(&self, parent: *mut u8) -> Result<*mut u8, StorageError>;

    fn validates(&self) -> bool {
        false
    }

    /// The error every write through this step reports, checked after the
    /// root's write authority and before any value is resolved mutably.
    fn write_rejection(&self) -> Option<StorageError> {
        None
    }

    /// Apply `mutate` to a copy of `value` reached through `suffix`, then
    /// validate the copy. The live value is never touched.
    ///
    /// # Safety
    /// `value` must point to a live value of [`Self::input_type`], and `suffix`
    /// must continue this step's output type.
    unsafe fn stage(
        &self,
        _value: *const u8,
        _suffix: &[Arc<dyn ErasedPathStep>],
        _mutate: &mut dyn FnMut(*mut u8),
    ) -> Result<(), StorageError> {
        Ok(())
    }

    /// # Safety
    /// `value` must point to a live value of [`Self::input_type`].
    unsafe fn check(&self, _value: *const u8) -> Result<(), StorageError> {
        Ok(())
    }
}

struct ValidatedStep<Field: 'static> {
    validate: ValidateField<Field>,
}

impl<Field: 'static> fmt::Debug for ValidatedStep<Field> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedStep")
            .field("field", &type_name::<Field>())
            .finish_non_exhaustive()
    }
}

impl<Field: Clone + 'static> ErasedPathStep for ValidatedStep<Field> {
    fn input_type(&self) -> TypeId {
        TypeId::of::<Field>()
    }

    fn output_type(&self) -> TypeId {
        TypeId::of::<Field>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same_identity(&self, other: &dyn ErasedPathStep) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.validate as usize == other.validate as usize)
    }

    unsafe fn project_ref(&self, parent: *const u8) -> Result<*const u8, StorageError> {
        Ok(parent)
    }

    unsafe fn project_mut(&self, parent: *mut u8) -> Result<*mut u8, StorageError> {
        Ok(parent)
    }

    fn validates(&self) -> bool {
        true
    }

    unsafe fn stage(
        &self,
        value: *const u8,
        suffix: &[Arc<dyn ErasedPathStep>],
        mutate: &mut dyn FnMut(*mut u8),
    ) -> Result<(), StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let mut candidate = unsafe { &*value.cast::<Field>() }.clone();
        let root = (&mut candidate as *mut Field).cast::<u8>();
        let mut leaf = root;
        for step in suffix {
            // SAFETY: the suffix continues this step's type chain over the
            // uniquely owned candidate.
            leaf = unsafe { step.project_mut(leaf)? };
        }
        mutate(leaf);
        let mut current = root.cast_const();
        for step in suffix {
            // SAFETY: as above, over the mutated candidate.
            unsafe {
                step.check(current)?;
                current = step.project_ref(current)?;
            }
        }
        (self.validate)(&candidate)
    }

    unsafe fn check(&self, value: *const u8) -> Result<(), StorageError> {
        // SAFETY: forwarded from the erased step contract.
        (self.validate)(unsafe { &*value.cast::<Field>() })
    }
}

struct InlineFieldStep<Parent: 'static, Field: 'static> {
    read: ReadField<Parent, Field>,
    write: WriteField<Parent, Field>,
}

impl<Parent: 'static, Field: 'static> fmt::Debug for InlineFieldStep<Parent, Field> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineFieldStep")
            .field("parent", &type_name::<Parent>())
            .field("field", &type_name::<Field>())
            .finish_non_exhaustive()
    }
}

impl<Parent: 'static, Field: 'static> ErasedPathStep for InlineFieldStep<Parent, Field> {
    fn input_type(&self) -> TypeId {
        TypeId::of::<Parent>()
    }

    fn output_type(&self) -> TypeId {
        TypeId::of::<Field>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same_identity(&self, other: &dyn ErasedPathStep) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|other| {
            self.read as usize == other.read as usize && self.write as usize == other.write as usize
        })
    }

    unsafe fn project_ref(&self, parent: *const u8) -> Result<*const u8, StorageError> {
        // SAFETY: forwarded from the erased step contract. The stored function
        // pointer returns a field contained by that parent reference.
        let parent = unsafe { &*parent.cast::<Parent>() };
        Ok((self.read)(parent) as *const Field as *const u8)
    }

    unsafe fn project_mut(&self, parent: *mut u8) -> Result<*mut u8, StorageError> {
        // SAFETY: forwarded from the erased step contract. The stored function
        // pointer returns a field contained by that unique parent reference.
        let parent = unsafe { &mut *parent.cast::<Parent>() };
        Ok((self.write)(parent) as *mut Field as *mut u8)
    }
}

struct OptionalFieldStep<Parent: 'static, Field: 'static> {
    name: &'static str,
    read: ReadField<Parent, Option<Field>>,
    write: WriteField<Parent, Option<Field>>,
}

impl<Parent: 'static, Field: 'static> fmt::Debug for OptionalFieldStep<Parent, Field> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OptionalFieldStep")
            .field("parent", &type_name::<Parent>())
            .field("field", &type_name::<Field>())
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl<Parent: 'static, Field: 'static> ErasedPathStep for OptionalFieldStep<Parent, Field> {
    fn input_type(&self) -> TypeId {
        TypeId::of::<Parent>()
    }

    fn output_type(&self) -> TypeId {
        TypeId::of::<Field>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same_identity(&self, other: &dyn ErasedPathStep) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|other| {
            self.name == other.name
                && self.read as usize == other.read as usize
                && self.write as usize == other.write as usize
        })
    }

    unsafe fn project_ref(&self, parent: *const u8) -> Result<*const u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &*parent.cast::<Parent>() };
        (self.read)(parent)
            .as_ref()
            .map(|value| value as *const Field as *const u8)
            .ok_or(StorageError::VariantChanged(self.name))
    }

    unsafe fn project_mut(&self, parent: *mut u8) -> Result<*mut u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &mut *parent.cast::<Parent>() };
        (self.write)(parent)
            .as_mut()
            .map(|value| value as *mut Field as *mut u8)
            .ok_or(StorageError::VariantChanged(self.name))
    }
}

struct EnumVariantStep<Parent: 'static, Field: 'static> {
    name: &'static str,
    read: ReadVariant<Parent, Field>,
    write: WriteVariant<Parent, Field>,
}

impl<Parent: 'static, Field: 'static> fmt::Debug for EnumVariantStep<Parent, Field> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnumVariantStep")
            .field("parent", &type_name::<Parent>())
            .field("field", &type_name::<Field>())
            .field("variant", &self.name)
            .finish_non_exhaustive()
    }
}

impl<Parent: 'static, Field: 'static> ErasedPathStep for EnumVariantStep<Parent, Field> {
    fn input_type(&self) -> TypeId {
        TypeId::of::<Parent>()
    }

    fn output_type(&self) -> TypeId {
        TypeId::of::<Field>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same_identity(&self, other: &dyn ErasedPathStep) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|other| {
            self.name == other.name
                && self.read as usize == other.read as usize
                && self.write as usize == other.write as usize
        })
    }

    unsafe fn project_ref(&self, parent: *const u8) -> Result<*const u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &*parent.cast::<Parent>() };
        (self.read)(parent)
            .map(|value| value as *const Field as *const u8)
            .ok_or(StorageError::VariantChanged(self.name))
    }

    unsafe fn project_mut(&self, parent: *mut u8) -> Result<*mut u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &mut *parent.cast::<Parent>() };
        (self.write)(parent)
            .map(|value| value as *mut Field as *mut u8)
            .ok_or(StorageError::VariantChanged(self.name))
    }
}

/// A projection through a getter-only upstream accessor such as
/// `Mesh::skinned_mesh_bounds`, which offers no mutable counterpart.
struct ReadOnlyAccessorStep<Parent: 'static, Field: 'static> {
    accessor: &'static str,
    setter: &'static str,
    read: ReadVariant<Parent, Field>,
}

impl<Parent: 'static, Field: 'static> fmt::Debug for ReadOnlyAccessorStep<Parent, Field> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadOnlyAccessorStep")
            .field("parent", &type_name::<Parent>())
            .field("field", &type_name::<Field>())
            .field("accessor", &self.accessor)
            .finish_non_exhaustive()
    }
}

impl<Parent: 'static, Field: 'static> ErasedPathStep for ReadOnlyAccessorStep<Parent, Field> {
    fn input_type(&self) -> TypeId {
        TypeId::of::<Parent>()
    }

    fn output_type(&self) -> TypeId {
        TypeId::of::<Field>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same_identity(&self, other: &dyn ErasedPathStep) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|other| {
            self.accessor == other.accessor && self.read as usize == other.read as usize
        })
    }

    unsafe fn project_ref(&self, parent: *const u8) -> Result<*const u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &*parent.cast::<Parent>() };
        (self.read)(parent)
            .map(|value| value as *const Field as *const u8)
            .ok_or(StorageError::VariantChanged(self.accessor))
    }

    unsafe fn project_mut(&self, _parent: *mut u8) -> Result<*mut u8, StorageError> {
        Err(self.rejection())
    }

    fn write_rejection(&self) -> Option<StorageError> {
        Some(self.rejection())
    }
}

impl<Parent: 'static, Field: 'static> ReadOnlyAccessorStep<Parent, Field> {
    fn rejection(&self) -> StorageError {
        StorageError::ReadOnlyAccessor {
            accessor: self.accessor,
            setter: self.setter,
        }
    }
}

struct IndexedStep<Parent: 'static, Field: 'static> {
    index: usize,
    read: ReadIndex<Parent, Field>,
    write: WriteIndex<Parent, Field>,
}

impl<Parent: 'static, Field: 'static> fmt::Debug for IndexedStep<Parent, Field> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexedStep")
            .field("parent", &type_name::<Parent>())
            .field("field", &type_name::<Field>())
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<Parent: 'static, Field: 'static> ErasedPathStep for IndexedStep<Parent, Field> {
    fn input_type(&self) -> TypeId {
        TypeId::of::<Parent>()
    }

    fn output_type(&self) -> TypeId {
        TypeId::of::<Field>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same_identity(&self, other: &dyn ErasedPathStep) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|other| {
            self.index == other.index
                && self.read as usize == other.read as usize
                && self.write as usize == other.write as usize
        })
    }

    unsafe fn project_ref(&self, parent: *const u8) -> Result<*const u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &*parent.cast::<Parent>() };
        (self.read)(parent, self.index)
            .map(|value| value as *const Field as *const u8)
            .ok_or(StorageError::IndexOutOfRange)
    }

    unsafe fn project_mut(&self, parent: *mut u8) -> Result<*mut u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &mut *parent.cast::<Parent>() };
        (self.write)(parent, self.index)
            .map(|value| value as *mut Field as *mut u8)
            .ok_or(StorageError::IndexOutOfRange)
    }
}

struct KeyedStep<Parent: 'static, Key: 'static, Field: 'static> {
    key: Key,
    read: ReadKey<Parent, Key, Field>,
    write: WriteKey<Parent, Key, Field>,
}

impl<Parent, Key, Field> fmt::Debug for KeyedStep<Parent, Key, Field>
where
    Parent: 'static,
    Key: fmt::Debug + 'static,
    Field: 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyedStep")
            .field("parent", &type_name::<Parent>())
            .field("field", &type_name::<Field>())
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl<Parent, Key, Field> ErasedPathStep for KeyedStep<Parent, Key, Field>
where
    Parent: 'static,
    Key: Clone + fmt::Debug + Eq + Send + Sync + 'static,
    Field: 'static,
{
    fn input_type(&self) -> TypeId {
        TypeId::of::<Parent>()
    }

    fn output_type(&self) -> TypeId {
        TypeId::of::<Field>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same_identity(&self, other: &dyn ErasedPathStep) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|other| {
            self.key == other.key
                && self.read as usize == other.read as usize
                && self.write as usize == other.write as usize
        })
    }

    unsafe fn project_ref(&self, parent: *const u8) -> Result<*const u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &*parent.cast::<Parent>() };
        (self.read)(parent, &self.key)
            .map(|value| value as *const Field as *const u8)
            .ok_or_else(|| StorageError::KeyNotFound(format!("{:?}", self.key)))
    }

    unsafe fn project_mut(&self, parent: *mut u8) -> Result<*mut u8, StorageError> {
        // SAFETY: forwarded from the erased step contract.
        let parent = unsafe { &mut *parent.cast::<Parent>() };
        (self.write)(parent, &self.key)
            .map(|value| value as *mut Field as *mut u8)
            .ok_or_else(|| StorageError::KeyNotFound(format!("{:?}", self.key)))
    }
}

type PathSteps = SmallVec<[Arc<dyn ErasedPathStep>; 4]>;

#[derive(Debug, Clone)]
pub(crate) struct AssetPath {
    root_type: TypeId,
    output_type: TypeId,
    steps: PathSteps,
}

impl AssetPath {
    pub(crate) fn root<Asset: 'static>() -> Self {
        Self {
            root_type: TypeId::of::<Asset>(),
            output_type: TypeId::of::<Asset>(),
            steps: SmallVec::new(),
        }
    }

    fn extend(&self, steps: &[Arc<dyn ErasedPathStep>]) -> Self {
        let mut output_type = self.output_type;
        for step in steps {
            assert_eq!(
                output_type,
                step.input_type(),
                "asset path step input must match its parent output"
            );
            output_type = step.output_type();
        }
        let mut combined = self.steps.clone();
        combined.extend(steps.iter().cloned());
        Self {
            root_type: self.root_type,
            output_type,
            steps: combined,
        }
    }

    pub(crate) unsafe fn project_ref(&self, root: *const u8) -> Result<*const u8, StorageError> {
        let mut current = root;
        for step in self.steps.iter() {
            // SAFETY: the root type is fixed at construction and append checks
            // that each step consumes the preceding step's output type.
            current = unsafe { step.project_ref(current)? };
        }
        Ok(current)
    }

    pub(crate) unsafe fn project_mut(&self, root: *mut u8) -> Result<*mut u8, StorageError> {
        let mut current = root;
        for step in self.steps.iter() {
            // SAFETY: the root is unique and the checked step chain preserves
            // the unique projection through every nested field.
            current = unsafe { step.project_mut(current)? };
        }
        Ok(current)
    }

    /// Reject a write through any step that forbids one.
    pub(crate) fn check_writable(&self) -> Result<(), StorageError> {
        match self.steps.iter().find_map(|step| step.write_rejection()) {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Stage a write through the outermost validated step on a copy.
    ///
    /// Without a staged mutation, a path containing a validator rejects the
    /// write, so a raw mutable resolution cannot bypass it.
    ///
    /// # Safety
    /// `root` must point to a live value of the path's root type.
    pub(crate) unsafe fn stage_write(
        &self,
        root: *const u8,
        mutate: Option<&mut dyn FnMut(*mut u8)>,
    ) -> Result<(), StorageError> {
        self.check_writable()?;
        let mut current = root;
        for (index, step) in self.steps.iter().enumerate() {
            if step.validates() {
                let mutate = mutate.ok_or(StorageError::ValidatedFieldWrite)?;
                // SAFETY: `current` is this step's live input and the suffix
                // continues its checked type chain.
                return unsafe { step.stage(current, &self.steps[index + 1..], mutate) };
            }
            // SAFETY: the root type and step chain are checked at construction.
            current = unsafe { step.project_ref(current)? };
        }
        Ok(())
    }

    fn same_identity(&self, other: &Self) -> bool {
        self.root_type == other.root_type
            && self.output_type == other.output_type
            && self.steps.len() == other.steps.len()
            && self
                .steps
                .iter()
                .zip(other.steps.iter())
                .all(|(left, right)| left.same_identity(&**right))
    }
}

pub(crate) struct ErasedResolvedRef {
    pub(crate) ptr: *const u8,
    pub(crate) guard: SourceReadGuard,
}

pub(crate) struct ErasedResolvedMut {
    pub(crate) ptr: *mut u8,
    pub(crate) guard: SourceWriteGuard,
}

trait AccessGate: Send + Sync {
    fn gate(&self) -> &AtomicIsize;
}

pub(crate) struct BorrowedPathGuard {
    owner: Arc<dyn AccessGate>,
    write: bool,
}

impl fmt::Debug for BorrowedPathGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedPathGuard")
            .field("write", &self.write)
            .finish_non_exhaustive()
    }
}

impl BorrowedPathGuard {
    fn acquire(owner: Arc<dyn AccessGate>, write: bool) -> Result<Self, StorageError> {
        owner
            .gate()
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                if write {
                    (count == 0).then_some(-1)
                } else {
                    (count >= 0).then(|| count.checked_add(1)).flatten()
                }
            })
            .map_err(|_| StorageError::BorrowedAccessConflict)?;
        Ok(Self { owner, write })
    }
}

impl Drop for BorrowedPathGuard {
    fn drop(&mut self) {
        if self.write {
            self.owner.gate().store(0, Ordering::Release);
        } else {
            self.owner.gate().fetch_sub(1, Ordering::Release);
        }
    }
}

#[derive(Debug)]
pub(crate) enum SourceReadGuard {
    Asset { _guard: AssetResourceReadGuard },
    Borrowed { _guard: BorrowedPathGuard },
}

#[derive(Debug)]
pub(crate) enum SourceWriteGuard {
    Asset { _guard: AssetResourceWriteGuard },
    Borrowed { _guard: BorrowedPathGuard },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SourceIdentity {
    Asset(TypeId, UntypedAssetId, usize),
    Borrowed(usize),
}

pub(crate) trait ErasedRevalidatingSource: fmt::Debug + Send + Sync {
    fn resolve_ref(&self) -> Result<ErasedResolvedRef, StorageError>;
    fn check_write_access(&self) -> Result<(), StorageError>;
    fn resolve_mut(
        &self,
        stage: Option<&mut dyn FnMut(*mut u8)>,
    ) -> Result<ErasedResolvedMut, StorageError>;
    fn append_step(&self, path: AssetPath) -> Arc<dyn ErasedRevalidatingSource>;
    fn clone_readonly(&self) -> Arc<dyn ErasedRevalidatingSource>;
    fn root_identity(&self) -> SourceIdentity;
    fn path(&self) -> &AssetPath;
}

#[derive(Debug)]
pub struct RevalidatingRef<'a, T> {
    ptr: *const T,
    _guard: SourceReadGuard,
    _lifetime: PhantomData<&'a T>,
}

impl<T> Deref for RevalidatingRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: resolution checked validity; the retained guard excludes conflicting access.
        unsafe { &*self.ptr }
    }
}

#[derive(Debug)]
pub struct RevalidatingMut<'a, T> {
    ptr: *mut T,
    _guard: SourceWriteGuard,
    _lifetime: PhantomData<&'a mut T>,
}

impl<T> Deref for RevalidatingMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the retained write guard uniquely authorizes this path.
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for RevalidatingMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: the retained write guard uniquely authorizes this path.
        unsafe { &mut *self.ptr }
    }
}

pub struct RevalidatingSource<T> {
    inner: Arc<dyn ErasedRevalidatingSource>,
    marker: PhantomData<fn() -> T>,
}

enum BorrowedRoot<T> {
    Read(BorrowedRef<T>),
    Write(BorrowedMut<T>),
    Component(RevalidatingField),
}

impl<T> BorrowedRoot<T> {
    fn read(&self) -> Result<&T, StorageError> {
        match self {
            Self::Read(root) => root.get(),
            Self::Write(root) => root.get(),
            Self::Component(root) => root.get::<T>(),
        }
    }

    fn check_write(&self) -> Result<(), StorageError> {
        match self {
            Self::Read(root) => {
                root.validity().check_read()?;
                Err(StorageError::ReadOnly)
            }
            Self::Write(root) => root.validity().check_write(),
            Self::Component(root) => root.check_write_access(),
        }
    }

    fn write_ptr(&self) -> Result<*mut u8, StorageError> {
        match self {
            Self::Read(_) => Err(StorageError::ReadOnly),
            Self::Write(root) => Ok(root.share().get_mut()? as *mut T as *mut u8),
            Self::Component(root) => Ok(root.clone().get_mut::<T>()? as *mut T as *mut u8),
        }
    }
}

struct BorrowedShared<T> {
    root: BorrowedRoot<T>,
    gate: AtomicIsize,
}

impl<T> AccessGate for BorrowedShared<T>
where
    Self: Send + Sync,
{
    fn gate(&self) -> &AtomicIsize {
        &self.gate
    }
}

struct BorrowedSource<T> {
    shared: Arc<BorrowedShared<T>>,
    path: AssetPath,
    readonly: bool,
}

impl<T> fmt::Debug for BorrowedSource<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedSource")
            .field("path", &self.path)
            .finish()
    }
}

impl<T: Send + Sync + 'static> ErasedRevalidatingSource for BorrowedSource<T> {
    fn resolve_ref(&self) -> Result<ErasedResolvedRef, StorageError> {
        let guard = BorrowedPathGuard::acquire(self.shared.clone(), false)?;
        let root = self.shared.root.read()? as *const T as *const u8;
        // SAFETY: the typed root checked validity and the path checks every variant.
        let ptr = unsafe { self.path.project_ref(root)? };
        Ok(ErasedResolvedRef {
            ptr,
            guard: SourceReadGuard::Borrowed { _guard: guard },
        })
    }

    fn check_write_access(&self) -> Result<(), StorageError> {
        self.shared.root.check_write()?;
        if self.readonly {
            return Err(StorageError::ReadOnly);
        }
        let _guard = BorrowedPathGuard::acquire(self.shared.clone(), true)?;
        let root = self.shared.root.read()? as *const T as *const u8;
        // SAFETY: the root remains live under the retained path guard.
        unsafe { self.path.project_ref(root)? };
        self.path.check_writable()
    }

    fn resolve_mut(
        &self,
        stage: Option<&mut dyn FnMut(*mut u8)>,
    ) -> Result<ErasedResolvedMut, StorageError> {
        let guard = BorrowedPathGuard::acquire(self.shared.clone(), true)?;
        let root = self.shared.root.read()? as *const T as *const u8;
        // SAFETY: validate the full path under the writer before marking any change.
        unsafe {
            self.path.project_ref(root)?;
        }
        if self.readonly {
            return Err(StorageError::ReadOnly);
        }
        self.shared.root.check_write()?;
        // SAFETY: the root was read under this writer; staging touches only a copy.
        unsafe { self.path.stage_write(root, stage)? };
        let root = self.shared.root.write_ptr()?;
        // SAFETY: write authority is checked; no callback intervenes after preflight.
        let ptr = unsafe { self.path.project_mut(root)? };
        Ok(ErasedResolvedMut {
            ptr,
            guard: SourceWriteGuard::Borrowed { _guard: guard },
        })
    }

    fn append_step(&self, path: AssetPath) -> Arc<dyn ErasedRevalidatingSource> {
        Arc::new(Self {
            shared: self.shared.clone(),
            path,
            readonly: self.readonly,
        })
    }

    fn clone_readonly(&self) -> Arc<dyn ErasedRevalidatingSource> {
        Arc::new(Self {
            shared: self.shared.clone(),
            path: self.path.clone(),
            readonly: true,
        })
    }

    fn root_identity(&self) -> SourceIdentity {
        SourceIdentity::Borrowed(Arc::as_ptr(&self.shared) as usize)
    }

    fn path(&self) -> &AssetPath {
        &self.path
    }
}

/// A borrowed root whose first projection builds the source in one step.
pub(crate) struct BorrowedRootSource<T> {
    shared: Arc<BorrowedShared<T>>,
}

impl<T: Send + Sync + 'static> BorrowedRootSource<T> {
    fn new(root: BorrowedRoot<T>) -> Self {
        Self {
            shared: Arc::new(BorrowedShared {
                root,
                gate: AtomicIsize::new(0),
            }),
        }
    }

    fn extend<Field: 'static>(
        &self,
        steps: &[Arc<dyn ErasedPathStep>],
    ) -> RevalidatingSource<Field> {
        RevalidatingSource::new(Arc::new(BorrowedSource {
            shared: self.shared.clone(),
            path: AssetPath::root::<T>().extend(steps),
            readonly: false,
        }))
    }

    #[cfg(test)]
    pub(crate) fn field<Field: 'static>(
        &self,
        read: ReadField<T, Field>,
        write: WriteField<T, Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[field_step(read, write)])
    }

    pub(crate) fn validated_field<Field: Clone + 'static>(
        &self,
        read: ReadField<T, Field>,
        write: WriteField<T, Field>,
        validate: ValidateField<Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[field_step(read, write), validated_step(validate)])
    }

    pub(crate) fn variant<Field: 'static>(
        &self,
        name: &'static str,
        read: ReadVariant<T, Field>,
        write: WriteVariant<T, Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[variant_step(name, read, write)])
    }

    pub(crate) fn optional<Field: 'static>(
        &self,
        name: &'static str,
        read: ReadField<T, Option<Field>>,
        write: WriteField<T, Option<Field>>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[optional_step(name, read, write)])
    }

    pub(crate) fn read_only_accessor<Field: 'static>(
        &self,
        accessor: &'static str,
        setter: &'static str,
        read: ReadVariant<T, Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[read_only_accessor_step(accessor, setter, read)])
    }

    pub(crate) fn index<Field: 'static>(
        &self,
        index: usize,
        read: ReadIndex<T, Field>,
        write: WriteIndex<T, Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[index_step(index, read, write)])
    }
}

fn field_step<Parent: 'static, Field: 'static>(
    read: ReadField<Parent, Field>,
    write: WriteField<Parent, Field>,
) -> Arc<dyn ErasedPathStep> {
    Arc::new(InlineFieldStep { read, write })
}

fn validated_step<Field: Clone + 'static>(
    validate: ValidateField<Field>,
) -> Arc<dyn ErasedPathStep> {
    Arc::new(ValidatedStep { validate })
}

fn variant_step<Parent: 'static, Field: 'static>(
    name: &'static str,
    read: ReadVariant<Parent, Field>,
    write: WriteVariant<Parent, Field>,
) -> Arc<dyn ErasedPathStep> {
    Arc::new(EnumVariantStep { name, read, write })
}

fn optional_step<Parent: 'static, Field: 'static>(
    name: &'static str,
    read: ReadField<Parent, Option<Field>>,
    write: WriteField<Parent, Option<Field>>,
) -> Arc<dyn ErasedPathStep> {
    Arc::new(OptionalFieldStep { name, read, write })
}

fn read_only_accessor_step<Parent: 'static, Field: 'static>(
    accessor: &'static str,
    setter: &'static str,
    read: ReadVariant<Parent, Field>,
) -> Arc<dyn ErasedPathStep> {
    Arc::new(ReadOnlyAccessorStep {
        accessor,
        setter,
        read,
    })
}

fn index_step<Parent: 'static, Field: 'static>(
    index: usize,
    read: ReadIndex<Parent, Field>,
    write: WriteIndex<Parent, Field>,
) -> Arc<dyn ErasedPathStep> {
    Arc::new(IndexedStep { index, read, write })
}

impl<T: Copy + Send + Sync + 'static> RevalidatingSource<T> {
    pub(crate) fn from_borrowed_value(storage: &ValueStorage<T>) -> Option<BorrowedRootSource<T>> {
        let root = match &storage.inner {
            ValueStorageInner::BorrowedRef(root) => BorrowedRoot::Read(root.clone()),
            ValueStorageInner::BorrowedMut(root) => BorrowedRoot::Write(root.share()),
            ValueStorageInner::Revalidating(root) => BorrowedRoot::Component((**root).clone()),
            _ => return None,
        };
        Some(BorrowedRootSource::new(root))
    }
}

impl<T: Clone + Send + Sync + 'static> RevalidatingSource<T> {
    pub(crate) fn from_borrowed_field(storage: &FieldStorage<T>) -> Option<BorrowedRootSource<T>> {
        let root = match &storage.inner {
            FieldStorageInner::BorrowedRef(root) => BorrowedRoot::Read(root.clone()),
            FieldStorageInner::BorrowedMut(root) => BorrowedRoot::Write(root.share()),
            FieldStorageInner::Revalidating(root) => BorrowedRoot::Component((**root).clone()),
            _ => return None,
        };
        Some(BorrowedRootSource::new(root))
    }
}

impl<T: Asset> RevalidatingSource<T> {
    pub(crate) fn from_borrowed_asset(storage: &AssetStorage<T>) -> Option<BorrowedRootSource<T>> {
        let root = match &storage.inner {
            AssetStorageInner::BorrowedRef { borrow, .. } => BorrowedRoot::Read(borrow.clone()),
            AssetStorageInner::BorrowedMut { borrow, .. } => BorrowedRoot::Write(borrow.share()),
            _ => return None,
        };
        Some(BorrowedRootSource::new(root))
    }
}

impl<T: Component> RevalidatingSource<T> {
    pub(crate) fn from_borrowed_component(
        storage: &ComponentStorage<T>,
    ) -> Option<BorrowedRootSource<T>> {
        let root = match &storage.inner {
            ComponentStorageInner::BorrowedRef(root) => BorrowedRoot::Read(root.clone()),
            ComponentStorageInner::BorrowedMut(root) => BorrowedRoot::Write(root.share()),
            ComponentStorageInner::Revalidating(root) => BorrowedRoot::Component((**root).clone()),
            _ => return None,
        };
        Some(BorrowedRootSource::new(root))
    }
}

impl<T> fmt::Debug for RevalidatingSource<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RevalidatingSource")
            .field("value", &type_name::<T>())
            .field("path", self.inner.path())
            .finish_non_exhaustive()
    }
}

impl<T> Clone for RevalidatingSource<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone_readonly(),
            marker: PhantomData,
        }
    }
}

impl<T: 'static> RevalidatingSource<T> {
    pub(crate) fn new(inner: Arc<dyn ErasedRevalidatingSource>) -> Self {
        debug_assert_eq!(inner.path().output_type, TypeId::of::<T>());
        Self {
            inner,
            marker: PhantomData,
        }
    }

    fn extend<Field: 'static>(
        &self,
        steps: &[Arc<dyn ErasedPathStep>],
    ) -> RevalidatingSource<Field> {
        let path = self.inner.path().extend(steps);
        RevalidatingSource::new(self.inner.append_step(path))
    }

    pub fn field<Field: 'static>(
        &self,
        read: ReadField<T, Field>,
        write: WriteField<T, Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[field_step(read, write)])
    }

    pub fn variant<Field: 'static>(
        &self,
        name: &'static str,
        read: ReadVariant<T, Field>,
        write: WriteVariant<T, Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[variant_step(name, read, write)])
    }

    /// Project the `Some` payload of an `Option` field, rechecked on every access.
    pub fn optional<Field: 'static>(
        &self,
        name: &'static str,
        read: ReadField<T, Option<Field>>,
        write: WriteField<T, Option<Field>>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[optional_step(name, read, write)])
    }

    /// Project through a getter-only accessor returning `Option<&Field>`.
    /// Every write through the result reports `ReadOnlyAccessor`, naming
    /// `setter` as the way to replace the value.
    pub fn read_only_accessor<Field: 'static>(
        &self,
        accessor: &'static str,
        setter: &'static str,
        read: ReadVariant<T, Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[read_only_accessor_step(accessor, setter, read)])
    }

    pub fn index<Field: 'static>(
        &self,
        index: usize,
        read: ReadIndex<T, Field>,
        write: WriteIndex<T, Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[index_step(index, read, write)])
    }

    /// Check every staged write through this path with `validate`.
    pub fn validated(&self, validate: ValidateField<T>) -> RevalidatingSource<T>
    where
        T: Clone,
    {
        self.extend(&[validated_step(validate)])
    }

    /// Project a field whose every staged write must pass `validate`.
    pub fn validated_field<Field: Clone + 'static>(
        &self,
        read: ReadField<T, Field>,
        write: WriteField<T, Field>,
        validate: ValidateField<Field>,
    ) -> RevalidatingSource<Field> {
        self.extend(&[field_step(read, write), validated_step(validate)])
    }

    pub fn key<Key, Field>(
        &self,
        key: Key,
        read: ReadKey<T, Key, Field>,
        write: WriteKey<T, Key, Field>,
    ) -> RevalidatingSource<Field>
    where
        Key: Clone + fmt::Debug + Eq + Send + Sync + 'static,
        Field: 'static,
    {
        self.extend(&[Arc::new(KeyedStep { key, read, write })])
    }
}

impl<T> RevalidatingSource<T> {
    pub fn check_write_access(&self) -> Result<(), StorageError> {
        self.inner.check_write_access()
    }

    pub fn resolve_ref(&self) -> Result<RevalidatingRef<'_, T>, StorageError> {
        let resolved = self.inner.resolve_ref()?;
        Ok(RevalidatingRef {
            ptr: resolved.ptr.cast::<T>(),
            _guard: resolved.guard,
            _lifetime: PhantomData,
        })
    }

    pub fn resolve_mut(&self) -> Result<RevalidatingMut<'_, T>, StorageError> {
        let resolved = self.inner.resolve_mut(None)?;
        Ok(RevalidatingMut {
            ptr: resolved.ptr.cast::<T>(),
            _guard: resolved.guard,
            _lifetime: PhantomData,
        })
    }

    /// Apply `mutate` once to a staged copy and, when every validator on the
    /// path accepts the result, once to the live value. A rejected write
    /// leaves the value and its change ticks untouched. `mutate` must depend
    /// only on the value it receives.
    pub fn update<R>(&self, mut mutate: impl FnMut(&mut T) -> R) -> Result<R, StorageError> {
        let mut stage = |value: *mut u8| {
            // SAFETY: the staged pointer addresses this path's output type.
            mutate(unsafe { &mut *value.cast::<T>() });
        };
        let resolved = self
            .inner
            .resolve_mut(Some(&mut stage as &mut dyn FnMut(*mut u8)))?;
        // SAFETY: the retained write guard uniquely authorizes this path.
        Ok(mutate(unsafe { &mut *resolved.ptr.cast::<T>() }))
    }

    pub fn same_identity(&self, other: &Self) -> bool {
        self.inner.root_identity() == other.inner.root_identity()
            && self.inner.path().same_identity(other.inner.path())
    }
}

// SAFETY: erased roots check thread-affine validity and retain their matching access guard.
unsafe impl<T> Send for RevalidatingSource<T> {}
// SAFETY: another thread cannot pass the source's validity check.
unsafe impl<T> Sync for RevalidatingSource<T> {}

#[cfg(test)]
mod tests {
    use bevy::ecs::{change_detection::Tick, component::Component, entity::Entity, world::World};

    use super::*;
    use crate::{AccessMode, BorrowableStorage, ValidityFlag};

    #[derive(Component, Clone, Copy)]
    struct Holder(Option<f32>);

    #[derive(Component)]
    struct Extra;

    #[derive(Component, Clone, Copy, Debug, PartialEq)]
    struct Span {
        bounds: (f32, f32),
        label: u8,
    }

    fn ordered(bounds: &(f32, f32)) -> Result<(), StorageError> {
        if bounds.0 > bounds.1 {
            return Err(StorageError::InvalidValue(
                "start must not exceed end".into(),
            ));
        }
        Ok(())
    }

    fn changed_tick(world: &World, entity: Entity) -> Tick {
        let component = world.components().component_id::<Span>().unwrap();
        world
            .entity(entity)
            .get_change_ticks_by_id(component)
            .unwrap()
            .changed
    }

    #[test]
    fn revalidating_payload_survives_moves_and_failed_write_does_not_mark() {
        let mut world = World::new();
        let entity = world.spawn(Holder(Some(1.0))).id();
        let component = world.components().component_id::<Holder>().unwrap();
        let flag = ValidityFlag::new_write();
        // SAFETY: the test owns World and accesses only this live Holder through the handle.
        let storage: ValueStorage<Holder> = unsafe {
            ValueStorage::revalidating(
                world.as_unsafe_world_cell(),
                entity,
                component,
                0,
                flag.with_access_mode(AccessMode::Write),
            )
        };
        let source = RevalidatingSource::from_borrowed_value(&storage).unwrap();
        let payload = source.variant("Some", |root| root.0.as_ref(), |root| root.0.as_mut());
        world.entity_mut(entity).insert(Extra);
        *payload.resolve_mut().unwrap() = 2.0;
        assert_eq!(world.get::<Holder>(entity).unwrap().0, Some(2.0));
        world.entity_mut(entity).insert(Holder(None));
        let before = world
            .entity(entity)
            .get_change_ticks_by_id(component)
            .unwrap()
            .changed;
        world.increment_change_tick();
        assert!(matches!(
            payload.resolve_mut(),
            Err(StorageError::VariantChanged("Some"))
        ));
        let after = world
            .entity(entity)
            .get_change_ticks_by_id(component)
            .unwrap()
            .changed;
        assert_eq!(before, after);
        world.despawn(entity);
        assert!(matches!(
            payload.resolve_ref(),
            Err(StorageError::EntityUnavailable)
        ));
        flag.set_invalid();
    }

    #[test]
    fn borrowed_variant_checks_authority_guards_and_discriminant() {
        let mut value = Some(1.0_f32);
        let flag = ValidityFlag::new_write();
        // SAFETY: value lives through all accesses on this thread under flag.
        let mut storage = unsafe { ValueStorage::borrowed_mut(&mut value, flag.clone()) };
        let source = RevalidatingSource::from_borrowed_value(&storage).unwrap();
        let payload = source.variant("Some", Option::as_ref, Option::as_mut);
        {
            let first = payload.resolve_ref().unwrap();
            let second = payload.resolve_ref().unwrap();
            assert_eq!(*first, 1.0);
            assert_eq!(*second, 1.0);
            assert!(matches!(
                payload.resolve_mut(),
                Err(StorageError::BorrowedAccessConflict)
            ));
        }
        {
            let mut write = payload.resolve_mut().unwrap();
            *write = 2.0;
            assert!(matches!(
                payload.resolve_ref(),
                Err(StorageError::BorrowedAccessConflict)
            ));
            assert!(matches!(
                payload.resolve_mut(),
                Err(StorageError::BorrowedAccessConflict)
            ));
        }
        assert_eq!(storage.get().unwrap(), Some(2.0));
        let readonly = payload.clone();
        assert!(matches!(
            readonly.resolve_mut(),
            Err(StorageError::ReadOnly)
        ));
        *storage.as_mut().unwrap() = None;
        assert!(matches!(
            payload.resolve_ref(),
            Err(StorageError::VariantChanged("Some"))
        ));
        assert!(matches!(
            payload.resolve_mut(),
            Err(StorageError::VariantChanged("Some"))
        ));
        *storage.as_mut().unwrap() = Some(3.0);
        assert_eq!(*payload.resolve_ref().unwrap(), 3.0);
        flag.set_invalid();
        assert!(matches!(
            payload.resolve_ref(),
            Err(StorageError::InvalidAccess)
        ));
        assert!(matches!(
            payload.resolve_mut(),
            Err(StorageError::InvalidAccess)
        ));
    }

    #[test]
    fn validated_field_stages_writes_and_rejects_without_marking() {
        let mut world = World::new();
        let entity = world
            .spawn(Span {
                bounds: (0.25, 0.5),
                label: 1,
            })
            .id();
        let component = world.components().component_id::<Span>().unwrap();
        let flag = ValidityFlag::new_write();
        // SAFETY: the test owns World and accesses only this live Span through the handle.
        let storage: ValueStorage<Span> = unsafe {
            ValueStorage::revalidating(
                world.as_unsafe_world_cell(),
                entity,
                component,
                0,
                flag.with_access_mode(AccessMode::Write),
            )
        };
        let bounds = RevalidatingSource::from_borrowed_value(&storage)
            .unwrap()
            .field(|span| &span.bounds, |span| &mut span.bounds)
            .validated(ordered);
        let end = bounds.field(|bounds| &bounds.1, |bounds| &mut bounds.1);
        world.increment_change_tick();
        let before = changed_tick(&world, entity);

        assert!(matches!(
            end.update(|end| *end = 0.125),
            Err(StorageError::InvalidValue(_))
        ));
        assert!(matches!(
            bounds.resolve_mut(),
            Err(StorageError::ValidatedFieldWrite)
        ));
        assert!(matches!(
            end.resolve_mut(),
            Err(StorageError::ValidatedFieldWrite)
        ));
        assert_eq!(world.get::<Span>(entity).unwrap().bounds, (0.25, 0.5));
        assert_eq!(changed_tick(&world, entity), before);

        end.update(|end| *end = 0.75).unwrap();
        assert_eq!(world.get::<Span>(entity).unwrap().bounds, (0.25, 0.75));
        assert_ne!(changed_tick(&world, entity), before);
        bounds.update(|bounds| *bounds = (0.5, 0.5)).unwrap();
        assert_eq!(world.get::<Span>(entity).unwrap().bounds, (0.5, 0.5));
        assert_eq!(world.get::<Span>(entity).unwrap().label, 1);
        flag.set_invalid();
    }

    #[test]
    fn validated_read_only_path_reports_authority_before_the_value() {
        let span = Span {
            bounds: (0.25, 0.5),
            label: 1,
        };
        let flag = ValidityFlag::new_read();
        // SAFETY: span remains live and immutable for this validity window.
        let storage = unsafe { ValueStorage::borrowed_ref(&span, flag.clone()) };
        let end = RevalidatingSource::from_borrowed_value(&storage)
            .unwrap()
            .field(|span| &span.bounds, |span| &mut span.bounds)
            .validated(ordered)
            .field(|bounds| &bounds.1, |bounds| &mut bounds.1);

        assert!(matches!(
            end.update(|end| *end = 0.125),
            Err(StorageError::ReadOnly)
        ));
        flag.set_invalid();
        assert!(matches!(
            end.update(|end| *end = 0.75),
            Err(StorageError::InvalidAccess)
        ));
    }

    #[test]
    fn shared_borrow_payload_stays_readonly_and_owned_values_do_not_escape() {
        let value = Some(1.0_f32);
        let flag = ValidityFlag::new_read();
        // SAFETY: value remains live and immutable for this validity window.
        let storage = unsafe { ValueStorage::borrowed_ref(&value, flag.clone()) };
        let source = RevalidatingSource::from_borrowed_value(&storage).unwrap();
        let payload = source.variant("Some", Option::as_ref, Option::as_mut);
        assert_eq!(*payload.resolve_ref().unwrap(), 1.0);
        assert!(matches!(payload.resolve_mut(), Err(StorageError::ReadOnly)));
        flag.set_invalid();
        assert!(matches!(
            payload.resolve_ref(),
            Err(StorageError::InvalidAccess)
        ));
        assert!(RevalidatingSource::from_borrowed_value(&ValueStorage::owned(value)).is_none());
    }

    mod allocations {
        use std::{
            alloc::{GlobalAlloc, Layout, System},
            cell::Cell,
        };

        use super::*;
        use crate::{ComponentStorage, FromBorrowedStorage};

        struct CountingAllocator;

        thread_local! {
            static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
        }

        // SAFETY: forwards every call to the system allocator unchanged.
        unsafe impl GlobalAlloc for CountingAllocator {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
                // SAFETY: forwarded from the caller.
                unsafe { System.alloc(layout) }
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                // SAFETY: forwarded from the caller.
                unsafe { System.dealloc(ptr, layout) }
            }
        }

        #[global_allocator]
        static GLOBAL: CountingAllocator = CountingAllocator;

        fn allocations_of<R>(f: impl FnOnce() -> R) -> usize {
            let before = ALLOCATIONS.with(Cell::get);
            let result = f();
            let after = ALLOCATIONS.with(Cell::get);
            drop(result);
            after - before
        }

        struct Payload(ValueStorage<f32>);

        impl FromBorrowedStorage<ValueStorage<f32>> for Payload {
            fn from_borrowed(storage: ValueStorage<f32>) -> Self {
                Self(storage)
            }
        }

        struct Bounds(ValueStorage<(f32, f32)>);

        impl FromBorrowedStorage<ValueStorage<(f32, f32)>> for Bounds {
            fn from_borrowed(storage: ValueStorage<(f32, f32)>) -> Self {
                Self(storage)
            }
        }

        fn optional(storage: &ComponentStorage<Holder>) -> ValueStorage<f32> {
            storage
                .borrow_optional_field::<f32, ValueStorage<f32>>(
                    "Holder.0",
                    |holder| &holder.0,
                    |holder| &mut holder.0,
                )
                .unwrap()
                .unwrap()
        }

        fn variant(storage: &ComponentStorage<Holder>) -> Payload {
            storage
                .borrow_resolved_variant_as::<f32, ValueStorage<f32>, Payload>(
                    "Some",
                    |holder| holder.0.as_ref(),
                    |holder| holder.0.as_mut(),
                )
                .unwrap()
        }

        fn validated(storage: &ComponentStorage<Span>) -> Bounds {
            storage
                .borrow_validated_field_as::<(f32, f32), ValueStorage<(f32, f32)>, Bounds>(
                    |span| &span.bounds,
                    |span| &mut span.bounds,
                    ordered,
                )
                .unwrap()
        }

        #[test]
        fn checked_path_getters_allocate_within_budget() {
            let mut holder = Holder(Some(1.0));
            let mut span = Span {
                bounds: (0.25, 0.5),
                label: 1,
            };
            let flag = ValidityFlag::new_write();
            // SAFETY: both values outlive every access on this thread under flag.
            let holder_storage =
                unsafe { ComponentStorage::borrowed_mut(&mut holder, flag.clone()) };
            // SAFETY: as above.
            let span_storage = unsafe { ComponentStorage::borrowed_mut(&mut span, flag.clone()) };

            drop((
                optional(&holder_storage),
                variant(&holder_storage),
                validated(&span_storage),
            ));
            assert_eq!(allocations_of(|| optional(&holder_storage)), 3);
            assert_eq!(allocations_of(|| variant(&holder_storage)), 3);
            assert_eq!(allocations_of(|| validated(&span_storage)), 4);
            let child = optional(&holder_storage);
            assert_eq!(allocations_of(|| child.get().unwrap()), 0);
            assert_eq!(variant(&holder_storage).0.get().unwrap(), 1.0);
            assert_eq!(validated(&span_storage).0.get().unwrap(), (0.25, 0.5));
            flag.set_invalid();
        }
    }
}
