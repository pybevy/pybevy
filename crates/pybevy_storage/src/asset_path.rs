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

use bevy::asset::UntypedAssetId;

use crate::{
    AssetResourceReadGuard, AssetResourceWriteGuard, StorageError, ValueStorage,
    borrowed::{BorrowedMut, BorrowedRef, RevalidatingField},
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

#[derive(Debug, Clone)]
pub(crate) struct AssetPath {
    root_type: TypeId,
    output_type: TypeId,
    steps: Arc<[Arc<dyn ErasedPathStep>]>,
}

impl AssetPath {
    pub(crate) fn root<Asset: 'static>() -> Self {
        Self {
            root_type: TypeId::of::<Asset>(),
            output_type: TypeId::of::<Asset>(),
            steps: Arc::new([]),
        }
    }

    fn append(&self, step: Arc<dyn ErasedPathStep>) -> Self {
        assert_eq!(
            self.output_type,
            step.input_type(),
            "asset path step input must match its parent output"
        );
        let mut steps = self.steps.to_vec();
        let output_type = step.output_type();
        steps.push(step);
        Self {
            root_type: self.root_type,
            output_type,
            steps: steps.into(),
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

#[derive(Debug)]
pub(crate) struct BorrowedPathGuard {
    gate: Arc<AtomicIsize>,
    write: bool,
}

impl BorrowedPathGuard {
    fn acquire(gate: &Arc<AtomicIsize>, write: bool) -> Result<Self, StorageError> {
        gate.fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            if write {
                (count == 0).then_some(-1)
            } else {
                (count >= 0).then(|| count.checked_add(1)).flatten()
            }
        })
        .map_err(|_| StorageError::BorrowedAccessConflict)?;
        Ok(Self {
            gate: gate.clone(),
            write,
        })
    }
}

impl Drop for BorrowedPathGuard {
    fn drop(&mut self) {
        if self.write {
            self.gate.store(0, Ordering::Release);
        } else {
            self.gate.fetch_sub(1, Ordering::Release);
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
    fn resolve_mut(&self) -> Result<ErasedResolvedMut, StorageError>;
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

    fn write_ptr(&self) -> Result<*mut u8, StorageError> {
        match self {
            Self::Read(_) => Err(StorageError::ReadOnly),
            Self::Write(root) => Ok(root.share().get_mut()? as *mut T as *mut u8),
            Self::Component(root) => Ok(root.clone().get_mut::<T>()? as *mut T as *mut u8),
        }
    }
}

struct BorrowedSource<T> {
    root: Arc<BorrowedRoot<T>>,
    gate: Arc<AtomicIsize>,
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
        let guard = BorrowedPathGuard::acquire(&self.gate, false)?;
        let root = self.root.read()? as *const T as *const u8;
        // SAFETY: the typed root checked validity and the path checks every variant.
        let ptr = unsafe { self.path.project_ref(root)? };
        Ok(ErasedResolvedRef {
            ptr,
            guard: SourceReadGuard::Borrowed { _guard: guard },
        })
    }

    fn resolve_mut(&self) -> Result<ErasedResolvedMut, StorageError> {
        let guard = BorrowedPathGuard::acquire(&self.gate, true)?;
        let root = self.root.read()? as *const T as *const u8;
        // SAFETY: validate the full path under the writer before marking any change.
        unsafe {
            self.path.project_ref(root)?;
        }
        if self.readonly {
            return Err(StorageError::ReadOnly);
        }
        let root = self.root.write_ptr()?;
        // SAFETY: write authority is checked; no callback intervenes after preflight.
        let ptr = unsafe { self.path.project_mut(root)? };
        Ok(ErasedResolvedMut {
            ptr,
            guard: SourceWriteGuard::Borrowed { _guard: guard },
        })
    }

    fn append_step(&self, path: AssetPath) -> Arc<dyn ErasedRevalidatingSource> {
        Arc::new(Self {
            root: self.root.clone(),
            gate: self.gate.clone(),
            path,
            readonly: self.readonly,
        })
    }

    fn clone_readonly(&self) -> Arc<dyn ErasedRevalidatingSource> {
        Arc::new(Self {
            root: self.root.clone(),
            gate: self.gate.clone(),
            path: self.path.clone(),
            readonly: true,
        })
    }

    fn root_identity(&self) -> SourceIdentity {
        SourceIdentity::Borrowed(Arc::as_ptr(&self.root) as usize)
    }

    fn path(&self) -> &AssetPath {
        &self.path
    }
}

impl<T: Copy + Send + Sync + 'static> RevalidatingSource<T> {
    pub(crate) fn from_borrowed_value(storage: &ValueStorage<T>) -> Option<Self> {
        let root = match &storage.inner {
            ValueStorageInner::BorrowedRef(root) => BorrowedRoot::Read(root.clone()),
            ValueStorageInner::BorrowedMut(root) => BorrowedRoot::Write(root.share()),
            ValueStorageInner::Revalidating(root) => BorrowedRoot::Component((**root).clone()),
            _ => return None,
        };
        Some(Self::new(Arc::new(BorrowedSource {
            root: Arc::new(root),
            gate: Arc::new(AtomicIsize::new(0)),
            path: AssetPath::root::<T>(),
            readonly: false,
        })))
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

    pub fn field<Field: 'static>(
        &self,
        read: ReadField<T, Field>,
        write: WriteField<T, Field>,
    ) -> RevalidatingSource<Field> {
        let step: Arc<dyn ErasedPathStep> = Arc::new(InlineFieldStep { read, write });
        let path = self.inner.path().append(step);
        RevalidatingSource::new(self.inner.append_step(path))
    }

    pub fn variant<Field: 'static>(
        &self,
        name: &'static str,
        read: ReadVariant<T, Field>,
        write: WriteVariant<T, Field>,
    ) -> RevalidatingSource<Field> {
        let step: Arc<dyn ErasedPathStep> = Arc::new(EnumVariantStep { name, read, write });
        let path = self.inner.path().append(step);
        RevalidatingSource::new(self.inner.append_step(path))
    }

    pub fn index<Field: 'static>(
        &self,
        index: usize,
        read: ReadIndex<T, Field>,
        write: WriteIndex<T, Field>,
    ) -> RevalidatingSource<Field> {
        let step: Arc<dyn ErasedPathStep> = Arc::new(IndexedStep { index, read, write });
        let path = self.inner.path().append(step);
        RevalidatingSource::new(self.inner.append_step(path))
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
        let step: Arc<dyn ErasedPathStep> = Arc::new(KeyedStep { key, read, write });
        let path = self.inner.path().append(step);
        RevalidatingSource::new(self.inner.append_step(path))
    }
}

impl<T> RevalidatingSource<T> {
    pub fn resolve_ref(&self) -> Result<RevalidatingRef<'_, T>, StorageError> {
        let resolved = self.inner.resolve_ref()?;
        Ok(RevalidatingRef {
            ptr: resolved.ptr.cast::<T>(),
            _guard: resolved.guard,
            _lifetime: PhantomData,
        })
    }

    pub fn resolve_mut(&self) -> Result<RevalidatingMut<'_, T>, StorageError> {
        let resolved = self.inner.resolve_mut()?;
        Ok(RevalidatingMut {
            ptr: resolved.ptr.cast::<T>(),
            _guard: resolved.guard,
            _lifetime: PhantomData,
        })
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
    use bevy::ecs::{component::Component, world::World};

    use super::*;
    use crate::{AccessMode, BorrowableStorage, ValidityFlag};

    #[derive(Component, Clone, Copy)]
    struct Holder(Option<f32>);

    #[derive(Component)]
    struct Extra;

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
}
