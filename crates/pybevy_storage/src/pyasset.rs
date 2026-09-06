//! Generic asset storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for all PyBevy asset types,
//! eliminating code duplication across StandardMaterial, AnimationClip, Mesh, etc.
//!
//! # Safety Model
//!
//! PyBevy assets exist in two modes:
//!
//! ## 1. Owned Assets (Python-created)
//!
//! Created via Python constructors like `StandardMaterial()`, these assets are:
//! - **Stored in**: `Box<T>` on the heap (stable memory address)
//! - **Consumed by**: `Assets[T].add()` which takes ownership via `take()`
//! - **Valid when**: Before being added to `Assets<T>` storage
//!
//! ## 2. Borrowed Assets (ECS-stored)
//!
//! Retrieved via `Assets[T].get()` or `Assets[T].get_mut()`, these assets are:
//! - **Stored in**: Bevy's `Assets<T>` resource storage
//! - **Validity tracked by**: `ValidityFlagWithMode` (Arc<AtomicU8> + AccessMode)
//! - **Valid when**: During system execution only
//! - **Invalidated by**: `ValidityGuard` RAII when system completes
//!
//! ### Read-Only vs Mutable Borrowed Access
//!
//! Assets support two borrowing modes with **compile-time pointer provenance**:
//!
//! - **`BorrowedRef`**: Created from `Res[Assets[T]].get()`, wraps a `BorrowedRef<T>` (`*const T`)
//! - **`BorrowedMut`**: Created from `ResMut[Assets[T]].get_mut()`, wraps a `BorrowedMut<T>` (`*mut T`)

use std::{
    any::{TypeId, type_name},
    fmt,
    mem::transmute,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
    },
};

use bevy::{
    asset::{Asset, AssetId, Assets, UntypedAssetId},
    ecs::{change_detection::DetectChangesMut, world::unsafe_world_cell::UnsafeWorldCell},
};

use crate::{
    AssetAccessRegistry, AssetAccessScope, AssetPath, AssetResourceReadGuard, AssetResourceState,
    AssetResourceWriteGuard, BorrowableStorage, ErasedResolvedMut, ErasedResolvedRef,
    ErasedRevalidatingSource, FromBorrowedStorage, PendingViewClaim, ReadField, ReadViewClaim,
    RevalidatingSource, StorageMut, StorageRef, ValidityFlag, ValidityFlagWithMode, ViewCounters,
    WriteField,
    borrowed::{BorrowedMut, BorrowedRef},
    storage_error::StorageError,
};

/// One validity-bound access scope for borrowed wrappers from `Assets<T>`.
#[derive(Debug, Clone)]
pub struct AssetBorrowCounter {
    scope: AssetAccessScope,
}

impl Default for AssetBorrowCounter {
    fn default() -> Self {
        let registry = AssetAccessRegistry::default();
        let scope = registry.new_scope(
            TypeId::of::<()>(),
            "test asset",
            ValidityFlag::new_write(),
            "standalone",
        );
        Self::from_scope(scope)
    }
}

impl AssetBorrowCounter {
    pub fn from_scope(scope: AssetAccessScope) -> Self {
        Self { scope }
    }

    pub fn active(&self) -> usize {
        self.scope.active()
    }

    /// The shared view counters for `asset_id` in this scope (created on first
    /// use). All wrappers for one asset get counters sharing the same atomics.
    ///
    /// Registering a new asset periodically drops entries nothing holds any
    /// more. A scope that outlives one system run (a Python-owned `World`)
    /// would otherwise keep one entry per asset it ever borrowed. Only entries
    /// whose last wrapper and last zero-copy view are gone are dropped, so the
    /// exclusion an entry provides can never be lost while it still matters.
    ///
    /// Sweeping on *every* new asset would be quadratic where nothing is
    /// prunable: `Assets::__iter__` materializes a wrapper for every asset at
    /// once, so each registration would rescan an ever-larger live map.
    pub fn views_for(&self, asset_id: UntypedAssetId) -> ViewCounters {
        self.scope.resource_state().views_for(asset_id, &self.scope)
    }

    pub fn has_active(&self) -> bool {
        self.active() > 0
    }

    fn lease(&self, asset_id: UntypedAssetId) -> Result<AssetBorrowLease, StorageError> {
        let asset_key = self
            .scope
            .acquire(asset_id)
            .ok_or(StorageError::InvalidAccess)?;
        Ok(AssetBorrowLease {
            scope: self.scope.clone(),
            asset_key,
        })
    }

    pub fn scope(&self) -> &AssetAccessScope {
        &self.scope
    }
}

#[derive(Debug)]
struct AssetBorrowLease {
    scope: AssetAccessScope,
    asset_key: u64,
}

#[derive(Debug)]
struct AssetResolverRoot<T: Asset> {
    world: UnsafeWorldCell<'static>,
    asset_id: AssetId<T>,
    validity: ValidityFlag,
    state: Arc<AssetResourceState>,
    cached_ptr: AtomicPtr<T>,
    cached_epoch: AtomicU64,
    changed: AtomicBool,
}

// SAFETY: every cell access is fenced by the thread-affine validity flag and a
// resource-wide access guard. Construction requires the exact scheduler access
// for `Assets<T>`.
unsafe impl<T: Asset> Send for AssetResolverRoot<T> {}
// SAFETY: a shared wrapper cannot pass the validity check from another thread,
// and the atomics only cache a pointer/epoch pair after guarded resolution.
unsafe impl<T: Asset> Sync for AssetResolverRoot<T> {}

#[derive(Debug)]
struct AssetResolver<T: Asset> {
    root: Arc<AssetResolverRoot<T>>,
    writable: bool,
}

impl<T: Asset> AssetResolver<T> {
    unsafe fn new(
        world: UnsafeWorldCell<'_>,
        asset_id: AssetId<T>,
        validity: ValidityFlag,
        state: Arc<AssetResourceState>,
        ptr: *const T,
        writable: bool,
    ) -> Self {
        // SAFETY: the constructor contract ties this lifetime-erased cell to
        // the same validity flag stored in the resolver.
        let world = unsafe { transmute::<UnsafeWorldCell<'_>, UnsafeWorldCell<'static>>(world) };
        let epoch = state.epoch();
        Self {
            root: Arc::new(AssetResolverRoot {
                world,
                asset_id,
                validity,
                state,
                cached_ptr: AtomicPtr::new(ptr.cast_mut()),
                cached_epoch: AtomicU64::new(epoch),
                changed: AtomicBool::new(false),
            }),
            writable,
        }
    }

    fn resolve_read(&self) -> Result<(*const T, AssetResourceReadGuard), StorageError> {
        self.root.validity.check_read()?;
        let guard = self.root.state.try_read()?;
        if self.root.state.has_write_views(self.root.asset_id.into()) {
            return Err(StorageError::AssetViewsLive);
        }
        let epoch = self.root.state.epoch();
        let ptr = if AssetResourceState::epoch_is_cacheable(epoch)
            && self.root.cached_epoch.load(Ordering::Acquire) == epoch
        {
            self.root.cached_ptr.load(Ordering::Acquire)
        } else {
            // SAFETY: construction requires shared scheduler access to this
            // exact `Assets<T>` resource, and `guard` excludes mutable Python
            // resolution for the returned reference's lifetime.
            let assets = unsafe { self.root.world.get_resource::<Assets<T>>() }
                .ok_or(StorageError::AssetUnavailable)?;
            let asset = assets
                .get(self.root.asset_id)
                .ok_or(StorageError::AssetUnavailable)?;
            let ptr = asset as *const T as *mut T;
            self.root.cached_ptr.store(ptr, Ordering::Release);
            self.root.cached_epoch.store(epoch, Ordering::Release);
            ptr
        };
        Ok((ptr, guard))
    }

    fn clone_readonly(&self) -> Self {
        Self {
            root: self.root.clone(),
            writable: false,
        }
    }

    fn clone_authority(&self) -> Self {
        Self {
            root: self.root.clone(),
            writable: self.writable,
        }
    }

    fn resolve_read_under_write(
        &self,
        guard: &AssetResourceWriteGuard,
    ) -> Result<*const T, StorageError> {
        if !guard.authorizes(&self.root.state) {
            return Err(StorageError::AssetAccessConflict);
        }
        // SAFETY: the matching write guard exclusively authorizes access to
        // this exact `Assets<T>` resource.
        let assets = unsafe { self.root.world.get_resource::<Assets<T>>() }
            .ok_or(StorageError::AssetUnavailable)?;
        let asset = assets
            .get(self.root.asset_id)
            .ok_or(StorageError::AssetUnavailable)?;
        Ok(asset as *const T)
    }

    fn resolve_mut(&self) -> Result<(*mut T, AssetResourceWriteGuard), StorageError> {
        self.root.validity.check_write()?;
        if !self.writable {
            return Err(StorageError::AssetReadOnly);
        }
        let guard = self.root.state.try_write()?;
        if self.root.state.has_views(self.root.asset_id.into()) {
            return Err(StorageError::AssetViewsLive);
        }

        // Preflight the complete root path without marking the asset changed.
        self.resolve_read_under_write(&guard)?;

        let ptr = self.commit_mut_prevalidated(&guard);
        Ok((ptr, guard))
    }

    fn commit_mut_prevalidated(&self, guard: &AssetResourceWriteGuard) -> *mut T {
        assert!(
            guard.authorizes(&self.root.state),
            "asset write transaction guard must match its resolver"
        );
        // SAFETY: construction requires mutable scheduler access to this exact
        // resource, and `guard` excludes every other Python resolver.
        let mut assets = unsafe { self.root.world.get_resource_mut::<Assets<T>>() }
            .expect("preflight confirmed the Assets resource exists");
        let epoch = self.root.state.advance_epoch();
        assets.set_changed();
        let assets = assets.bypass_change_detection();
        let already_changed = self.root.changed.load(Ordering::Acquire);
        let ptr = if already_changed {
            assets
                .get_mut_untracked(self.root.asset_id)
                .expect("preflight confirmed the asset exists") as *mut T
        } else {
            let mut asset = assets
                .get_mut(self.root.asset_id)
                .expect("preflight confirmed the asset exists");
            let asset: &mut T = &mut asset;
            self.root.changed.store(true, Ordering::Release);
            asset as *mut T
        };
        self.root.cached_ptr.store(ptr, Ordering::Release);
        self.root.cached_epoch.store(epoch, Ordering::Release);
        ptr
    }
}

struct AssetResolverSource<A: Asset> {
    resolver: AssetResolver<A>,
    path: AssetPath,
    lease: Option<AssetBorrowLease>,
}

impl<A: Asset> fmt::Debug for AssetResolverSource<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetResolverSource")
            .field("asset", &type_name::<A>())
            .field("asset_id", &self.resolver.root.asset_id)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl<A: Asset> AssetResolverSource<A> {
    fn clone_with(&self, resolver: AssetResolver<A>, path: AssetPath) -> Self {
        Self {
            resolver,
            path,
            lease: self.lease.as_ref().and_then(AssetBorrowLease::try_clone),
        }
    }
}

impl<A: Asset> ErasedRevalidatingSource for AssetResolverSource<A> {
    fn resolve_ref(&self) -> Result<ErasedResolvedRef, StorageError> {
        let (root, guard) = self.resolver.resolve_read()?;
        // SAFETY: the resolver returned a live `A` under the retained guard;
        // `AssetPath` validates the typed step chain when it is built.
        let ptr = unsafe { self.path.project_ref(root.cast::<u8>())? };
        Ok(ErasedResolvedRef { ptr, guard })
    }

    fn resolve_mut(&self) -> Result<ErasedResolvedMut, StorageError> {
        self.resolver.root.validity.check_write()?;
        if !self.resolver.writable {
            return Err(StorageError::AssetReadOnly);
        }
        let guard = self.resolver.root.state.try_write()?;
        if self
            .resolver
            .root
            .state
            .has_views(self.resolver.root.asset_id.into())
        {
            return Err(StorageError::AssetViewsLive);
        }

        let root = self.resolver.resolve_read_under_write(&guard)?;
        // SAFETY: the root is live under this write guard. Reapplying the full
        // read path validates every fallible segment before marking.
        unsafe { self.path.project_ref(root.cast::<u8>())? };

        let root = self.resolver.commit_mut_prevalidated(&guard);
        // SAFETY: no code can change the asset while the resource writer is
        // retained, so the preflighted path must remain valid at commit.
        let ptr = unsafe {
            self.path
                .project_mut(root.cast::<u8>())
                .expect("asset path stayed valid under its write guard")
        };
        Ok(ErasedResolvedMut { ptr, guard })
    }

    fn append_step(&self, path: AssetPath) -> Arc<dyn ErasedRevalidatingSource> {
        Arc::new(self.clone_with(self.resolver.clone_authority(), path))
    }

    fn clone_readonly(&self) -> Arc<dyn ErasedRevalidatingSource> {
        Arc::new(self.clone_with(self.resolver.clone_readonly(), self.path.clone()))
    }

    fn root_identity(&self) -> (TypeId, UntypedAssetId, usize) {
        (
            TypeId::of::<A>(),
            self.resolver.root.asset_id.into(),
            Arc::as_ptr(&self.resolver.root.state) as usize,
        )
    }

    fn path(&self) -> &AssetPath {
        &self.path
    }
}

enum AssetViewWriteInner<'a, T: Asset> {
    Direct(&'a mut T),
    Resolved {
        resolver: &'a mut AssetResolver<T>,
        guard: AssetResourceWriteGuard,
        preflight: *const T,
    },
}

/// A preflighted mutable zero-copy view transaction.
///
/// Callers inspect [`preflight`](Self::preflight) before committing. Once
/// committed, the returned asset is reached through a real Bevy `AssetMut` and
/// no recoverable work may remain before the child view is published.
pub struct AssetViewWrite<'a, T: Asset> {
    inner: AssetViewWriteInner<'a, T>,
    committed: bool,
}

impl<T: Asset> AssetViewWrite<'_, T> {
    pub fn preflight(&self) -> &T {
        match &self.inner {
            AssetViewWriteInner::Direct(value) => value,
            AssetViewWriteInner::Resolved { preflight, .. } => {
                // SAFETY: the transaction retains the matching resource write
                // guard, and construction validated this pointer under it.
                unsafe { &**preflight }
            }
        }
    }

    pub fn commit(&mut self) -> &mut T {
        assert!(!self.committed, "asset view transaction commits once");
        self.committed = true;
        match &mut self.inner {
            AssetViewWriteInner::Direct(value) => value,
            AssetViewWriteInner::Resolved {
                resolver, guard, ..
            } => {
                let ptr = resolver.commit_mut_prevalidated(guard);
                // SAFETY: the retained write guard uniquely authorizes the
                // freshly derived asset pointer for this transaction.
                unsafe { &mut *ptr }
            }
        }
    }
}

impl AssetBorrowLease {
    fn try_clone(&self) -> Option<Self> {
        self.scope.acquire_existing(self.asset_key).then(|| Self {
            scope: self.scope.clone(),
            asset_key: self.asset_key,
        })
    }
}

impl Drop for AssetBorrowLease {
    fn drop(&mut self) {
        self.scope.release(self.asset_key);
    }
}

/// Generic storage for PyBevy assets
///
/// Supports two modes:
/// - `Owned`: Python-created asset, fully owned by Python
/// - `Borrowed`: Reference to asset in Bevy's `Assets<T>` storage
///
/// # Type Parameters
/// - `T`: The Bevy asset type (must implement `Asset + Clone`)
///
/// # Safety
/// Borrowed variant contains a raw pointer to asset data in `Assets<T>`.
/// The `ValidityFlag` ensures this pointer is only dereferenced during
/// system execution when the pointer is guaranteed to be valid.
#[derive(Debug)]
pub struct AssetStorage<T: Asset> {
    pub(crate) inner: AssetStorageInner<T>,
    views: ViewCounters,
    borrow_lease: Option<AssetBorrowLease>,
    resolver: Option<AssetResolver<T>>,
}

#[derive(Debug)]
pub(crate) enum AssetStorageInner<T: Asset> {
    /// Python-created instance, fully owned
    /// Option allows consuming the asset via take() when adding to Assets<T>
    Owned(Option<Box<T>>),

    /// Read-only snapshot of an asset-shaped field extracted from owned asset
    /// storage. The snapshot can be inspected but not consumed or mutated.
    OwnedReadOnly(Box<T>),

    /// Read-only borrowed reference to asset in Assets<T> storage
    /// Created from `&T` - mutation through this pointer is UB
    BorrowedRef {
        /// Typed read-only borrow into the asset
        borrow: BorrowedRef<T>,

        id: UntypedAssetId,
    },

    /// Mutable borrowed reference to asset in Assets<T> storage
    /// Created from `&mut T` - mutation is sound
    BorrowedMut {
        /// Typed mutable borrow into the asset
        borrow: BorrowedMut<T>,

        id: UntypedAssetId,
    },

    /// An asset-shaped value nested inside another live Bevy asset.
    Source(Box<RevalidatingSource<T>>),
}

impl<T: Asset + Clone> Clone for AssetStorage<T> {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            AssetStorageInner::Owned(Some(asset)) => {
                AssetStorageInner::Owned(Some(Box::new((**asset).clone())))
            }
            AssetStorageInner::Owned(None) => AssetStorageInner::Owned(None),
            AssetStorageInner::OwnedReadOnly(asset) => {
                AssetStorageInner::OwnedReadOnly(Box::new((**asset).clone()))
            }
            AssetStorageInner::BorrowedRef { borrow, id } => AssetStorageInner::BorrowedRef {
                borrow: borrow.clone(),
                id: *id,
            },
            // A cloned mutable borrow downgrades to read-only to avoid aliasing.
            AssetStorageInner::BorrowedMut { borrow, id } => AssetStorageInner::BorrowedRef {
                borrow: borrow.clone_as_ref(),
                id: *id,
            },
            AssetStorageInner::Source(source) => {
                AssetStorageInner::Source(Box::new((**source).clone()))
            }
        };
        // Borrowed clones alias the same asset data, so they share counters;
        // an owned clone copies the data and starts with none.
        let views = match &self.inner {
            AssetStorageInner::Owned(_) | AssetStorageInner::OwnedReadOnly(_) => {
                ViewCounters::default()
            }
            _ => self.views.clone(),
        };
        let borrow_lease = match &self.inner {
            AssetStorageInner::Owned(_) | AssetStorageInner::OwnedReadOnly(_) => None,
            _ => self
                .borrow_lease
                .as_ref()
                .and_then(AssetBorrowLease::try_clone),
        };
        Self {
            inner,
            views,
            borrow_lease,
            resolver: self.resolver.as_ref().map(AssetResolver::clone_readonly),
        }
    }
}

impl<T: Asset> AssetStorage<T> {
    /// Whether two wrappers name the same asset, ignoring its value.
    ///
    /// The fallback for `PartialEq` when either side cannot be read.
    fn same_target(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (AssetStorageInner::Owned(None), AssetStorageInner::Owned(None)) => true,
            (AssetStorageInner::Source(a), AssetStorageInner::Source(b)) => a.same_identity(b),
            (
                AssetStorageInner::BorrowedRef { id: a, .. },
                AssetStorageInner::BorrowedRef { id: b, .. },
            )
            | (
                AssetStorageInner::BorrowedRef { id: a, .. },
                AssetStorageInner::BorrowedMut { id: b, .. },
            )
            | (
                AssetStorageInner::BorrowedMut { id: a, .. },
                AssetStorageInner::BorrowedRef { id: b, .. },
            )
            | (
                AssetStorageInner::BorrowedMut { id: a, .. },
                AssetStorageInner::BorrowedMut { id: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

impl<T: Asset + PartialEq> PartialEq for AssetStorage<T> {
    /// Compare the assets by value, whichever storage mode each side is in.
    ///
    /// Falls back to naming the same asset when either side cannot be read
    /// (consumed by `add()`, expired borrow, or aliased by a live write view),
    /// so `x == x` holds even then.
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => *a == *b,
            _ => self.same_target(other),
        }
    }
}

impl<T: Asset> AssetStorage<T> {
    /// Create owned asset storage
    pub fn owned(asset: T) -> Self {
        Self {
            inner: AssetStorageInner::Owned(Some(Box::new(asset))),
            views: ViewCounters::default(),
            borrow_lease: None,
            resolver: None,
        }
    }

    pub(crate) fn owned_readonly(asset: T) -> Self {
        Self {
            inner: AssetStorageInner::OwnedReadOnly(Box::new(asset)),
            views: ViewCounters::default(),
            borrow_lease: None,
            resolver: None,
        }
    }

    pub(crate) fn revalidating_source(source: RevalidatingSource<T>) -> Self {
        Self {
            inner: AssetStorageInner::Source(Box::new(source)),
            views: ViewCounters::default(),
            borrow_lease: None,
            resolver: None,
        }
    }

    /// Take ownership of the asset, consuming it
    ///
    /// Returns the owned asset if available, or an error if:
    /// - Asset was already consumed
    /// - Asset is borrowed (not owned)
    ///
    /// # Errors
    /// Returns `StorageError::AssetConsumed` if asset was already taken
    /// Returns `StorageError::AssetBorrowed` if asset is a borrowed reference
    pub fn take(&mut self) -> Result<T, StorageError> {
        self.views.check_no_views()?;
        match &mut self.inner {
            AssetStorageInner::Owned(opt) => opt
                .take()
                .map(|boxed| *boxed)
                .ok_or(StorageError::AssetConsumed),
            AssetStorageInner::OwnedReadOnly(_)
            | AssetStorageInner::BorrowedRef { .. }
            | AssetStorageInner::BorrowedMut { .. }
            | AssetStorageInner::Source(_) => Err(StorageError::AssetBorrowed),
        }
    }

    /// Create read-only borrowed asset storage from `&T`.
    ///
    /// The `ValidityFlagWithMode` mode is transport-only: this constructor always
    /// produces a read-only borrow regardless of the transported mode.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in `Assets<T>` storage
    /// - The pointer must remain valid while `validity` is non-Invalid
    /// - The returned storage MUST NOT be used for mutable access
    pub unsafe fn borrowed_readonly(
        ptr: *const T,
        validity: ValidityFlagWithMode,
        id: UntypedAssetId,
    ) -> Self {
        // SAFETY: forwarded from this constructor's contract.
        unsafe { Self::borrowed_readonly_inner(ptr, validity, id, None, ViewCounters::default()) }
    }

    /// Create a borrow-counter-tracked read-only asset wrapper.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in `Assets<T>` storage.
    /// - The pointer and asset must remain valid while `validity` is active.
    /// - No mutable reference may alias the asset during that scope.
    pub unsafe fn borrowed_readonly_tracked(
        ptr: *const T,
        world: UnsafeWorldCell<'_>,
        asset_id: UntypedAssetId,
        validity: ValidityFlagWithMode,
        borrow_counter: AssetBorrowCounter,
    ) -> Result<Self, StorageError> {
        let views = borrow_counter.views_for(asset_id);
        let lease = borrow_counter.lease(asset_id)?;
        // SAFETY: the caller guarantees that `ptr` identifies `asset_id` in
        // `Assets<T>` inside `world` for the active validity scope.
        let resolver = unsafe {
            AssetResolver::new(
                world,
                asset_id.typed::<T>(),
                validity.flag.clone(),
                borrow_counter.scope().resource_state().clone(),
                ptr,
                false,
            )
        };
        // SAFETY: forwarded from this constructor's contract.
        let mut storage =
            unsafe { Self::borrowed_readonly_inner(ptr, validity, asset_id, Some(lease), views) };
        storage.resolver = Some(resolver);
        Ok(storage)
    }

    unsafe fn borrowed_readonly_inner(
        ptr: *const T,
        validity: ValidityFlagWithMode,
        id: UntypedAssetId,
        borrow_lease: Option<AssetBorrowLease>,
        views: ViewCounters,
    ) -> Self {
        Self {
            inner: AssetStorageInner::BorrowedRef {
                // SAFETY: forwards this constructor's contract unchanged
                borrow: unsafe { BorrowedRef::new(ptr, validity.flag) },
                id,
            },
            views,
            borrow_lease,
            resolver: None,
        }
    }

    /// Create mutable borrowed asset storage from `&mut T`.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in `Assets<T>` storage, from `&mut T`
    /// - The pointer must remain valid while `validity` is non-Invalid
    pub unsafe fn borrowed_mut(
        ptr: *mut T,
        validity: ValidityFlagWithMode,
        id: UntypedAssetId,
    ) -> Self {
        // SAFETY: forwarded from this constructor's contract.
        unsafe { Self::borrowed_mut_inner(ptr, validity, id, None, None, ViewCounters::default()) }
    }

    /// Create a borrow-counter-tracked mutable asset wrapper with lazy Bevy
    /// change notification.
    ///
    /// # Safety
    /// - `ptr` must identify `asset_id` in `Assets<T>` inside `world`.
    /// - `world`, the `Assets<T>` resource, and the asset must remain valid
    ///   while `validity` is active.
    /// - No concurrent world or asset access may occur during that scope.
    pub unsafe fn borrowed_mut_tracked(
        ptr: *const T,
        world: UnsafeWorldCell<'_>,
        asset_id: AssetId<T>,
        validity: ValidityFlagWithMode,
        borrow_counter: AssetBorrowCounter,
    ) -> Result<Self, StorageError> {
        let views = borrow_counter.views_for(asset_id.into());
        let lease = borrow_counter.lease(asset_id.into())?;
        // SAFETY: the caller guarantees that `ptr` identifies `asset_id` in
        // `Assets<T>` inside `world` for the active validity scope.
        let resolver = unsafe {
            AssetResolver::new(
                world,
                asset_id,
                validity.flag.clone(),
                borrow_counter.scope().resource_state().clone(),
                ptr,
                true,
            )
        };
        // The wrapper starts from a shared pointer. Mutable provenance is
        // derived lazily by the resolver only when a write actually occurs.
        // SAFETY: the pointer is used read-only until the resolver derives
        // fresh mutable provenance, and the caller guarantees its validity.
        let mut storage = unsafe {
            Self::borrowed_readonly_inner(ptr, validity, asset_id.into(), Some(lease), views)
        };
        storage.resolver = Some(resolver);
        Ok(storage)
    }

    unsafe fn borrowed_mut_inner(
        ptr: *mut T,
        validity: ValidityFlagWithMode,
        id: UntypedAssetId,
        borrow_lease: Option<AssetBorrowLease>,
        resolver: Option<AssetResolver<T>>,
        views: ViewCounters,
    ) -> Self {
        Self {
            inner: AssetStorageInner::BorrowedMut {
                // SAFETY: forwards this constructor's contract unchanged
                borrow: unsafe { BorrowedMut::new(ptr, validity.flag) },
                id,
            },
            views,
            borrow_lease,
            resolver,
        }
    }

    /// Get immutable reference to the asset, checking validity
    ///
    /// # Errors
    /// Returns [`StorageError`] if the borrowed reference is no longer valid
    /// (for example, after its system execution context ends).
    #[inline(always)]
    pub fn as_ref(&self) -> Result<StorageRef<'_, T>, StorageError> {
        if let Some(resolver) = &self.resolver {
            let (ptr, guard) = resolver.resolve_read()?;
            return Ok(StorageRef::resolved(ptr, guard));
        }
        self.views.check_no_write_views()?;
        match &self.inner {
            AssetStorageInner::Owned(Some(asset)) => Ok(StorageRef::Direct(&**asset)),
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            AssetStorageInner::OwnedReadOnly(asset) => Ok(StorageRef::Direct(&**asset)),
            AssetStorageInner::BorrowedRef { borrow, .. } => borrow.get().map(StorageRef::Direct),
            AssetStorageInner::BorrowedMut { borrow, .. } => borrow.get().map(StorageRef::Direct),
            AssetStorageInner::Source(source) => source.resolve_ref().map(StorageRef::Source),
        }
    }

    /// Get mutable reference to the asset, checking validity
    ///
    /// # Errors
    /// Returns `StorageError` if the borrowed reference is no longer valid, or
    /// `AssetReadOnly` if the asset was borrowed read-only.
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<StorageMut<'_, T>, StorageError> {
        if let Some(resolver) = &mut self.resolver {
            let (ptr, guard) = resolver.resolve_mut()?;
            return Ok(StorageMut::resolved(ptr, guard));
        }
        self.views.check_no_views()?;
        match &mut self.inner {
            AssetStorageInner::Owned(Some(asset)) => Ok(StorageMut::Direct(&mut **asset)),
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            AssetStorageInner::OwnedReadOnly(_) => Err(StorageError::OwnedFieldReadOnly),
            AssetStorageInner::BorrowedRef { .. } => Err(StorageError::AssetReadOnly),
            AssetStorageInner::BorrowedMut { borrow, .. } => {
                borrow.get_mut().map(StorageMut::Direct)
            }
            AssetStorageInner::Source(source) => source.resolve_mut().map(StorageMut::Source),
        }
    }

    fn root_source(&self) -> Result<RevalidatingSource<T>, StorageError> {
        if let AssetStorageInner::Source(source) = &self.inner {
            return Ok((**source).clone());
        }
        let resolver = self
            .resolver
            .as_ref()
            .ok_or(StorageError::AssetUnavailable)?;
        resolver.root.validity.check_read()?;
        let lease = self
            .borrow_lease
            .as_ref()
            .and_then(AssetBorrowLease::try_clone)
            .ok_or(StorageError::InvalidAccess)?;
        Ok(RevalidatingSource::new(Arc::new(AssetResolverSource {
            resolver: resolver.clone_authority(),
            path: AssetPath::root::<T>(),
            lease: Some(lease),
        })))
    }

    /// Borrow an inline field from this asset.
    ///
    /// Tracked world assets return a typed re-resolving source. Owned assets
    /// return an explicit read-only snapshot, and legacy pointer-backed assets
    /// retain their existing validity-bound borrow behavior.
    pub fn borrow_field<F: Clone + 'static, S>(
        &self,
        read: ReadField<T, F>,
        write: WriteField<T, F>,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        if self.resolver.is_some() {
            // Resolve once so an expired or missing asset fails at the getter,
            // then retain only identity and the typed path in the child.
            let current = self.as_ref()?;
            read(&current);
            let source = self.root_source()?.field(read, write);
            return Ok(S::revalidating_source(source));
        }
        match &self.inner {
            AssetStorageInner::Owned(Some(asset)) => Ok(S::snapshot(read(asset))),
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            AssetStorageInner::OwnedReadOnly(asset) => Ok(S::snapshot(read(asset))),
            AssetStorageInner::BorrowedRef { borrow, .. } => borrow.borrow_field(read),
            AssetStorageInner::BorrowedMut { borrow, .. } => borrow.borrow_field(read),
            AssetStorageInner::Source(source) => {
                let current = source.resolve_ref()?;
                read(&current);
                Ok(S::revalidating_source(source.field(read, write)))
            }
        }
    }

    /// Borrow an asset-shaped inline field from this asset.
    pub fn borrow_asset_field<F: Asset + Clone + 'static>(
        &self,
        read: ReadField<T, F>,
        write: WriteField<T, F>,
    ) -> Result<AssetStorage<F>, StorageError> {
        if self.resolver.is_some() || matches!(self.inner, AssetStorageInner::Source(_)) {
            let current = self.as_ref()?;
            read(&current);
            return Ok(AssetStorage::revalidating_source(
                self.root_source()?.field(read, write),
            ));
        }
        match &self.inner {
            AssetStorageInner::Owned(Some(asset)) => {
                Ok(AssetStorage::owned_readonly(read(asset).clone()))
            }
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            AssetStorageInner::OwnedReadOnly(asset) => {
                Ok(AssetStorage::owned_readonly(read(asset).clone()))
            }
            AssetStorageInner::BorrowedRef { borrow, id } => {
                let field = read(borrow.get()?);
                // SAFETY: the field pointer is inside the checked parent borrow
                // and inherits its validity and read-only authority.
                Ok(unsafe {
                    AssetStorage::borrowed_readonly(
                        field as *const F,
                        borrow
                            .validity()
                            .clone()
                            .with_access_mode(crate::AccessMode::Read),
                        *id,
                    )
                })
            }
            AssetStorageInner::BorrowedMut { borrow, id } => {
                let base = borrow.get()?;
                let field = read(base);
                let offset = (field as *const F as usize).wrapping_sub(base as *const T as usize);
                // SAFETY: `offset` was measured from a field reference inside
                // the same live parent allocation.
                let field_ptr = unsafe { (borrow.as_ptr() as *mut u8).add(offset).cast::<F>() };
                // SAFETY: the field pointer derives from the parent's mutable
                // provenance and inherits its validity window.
                Ok(unsafe {
                    AssetStorage::borrowed_mut(
                        field_ptr,
                        borrow
                            .validity()
                            .clone()
                            .with_access_mode(crate::AccessMode::Write),
                        *id,
                    )
                })
            }
            AssetStorageInner::Source(_) => unreachable!("source handled above"),
        }
    }

    pub fn borrow_field_as<F: Clone + 'static, S, W>(
        &self,
        read: ReadField<T, F>,
        write: WriteField<T, F>,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        Ok(W::from_borrowed(self.borrow_field(read, write)?))
    }

    /// Materialize an explicit read-only snapshot of an asset field.
    pub fn snapshot_field_as<F: Clone + 'static, S, W>(
        &self,
        read: ReadField<T, F>,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: FromBorrowedStorage<S>,
    {
        let asset = self.as_ref()?;
        Ok(W::from_borrowed(S::snapshot(read(&asset))))
    }

    /// Begin a mutable zero-copy view transaction after the caller has claimed
    /// this storage's write-view counter.
    pub fn begin_write_view(
        &mut self,
        claim: &PendingViewClaim,
    ) -> Result<AssetViewWrite<'_, T>, StorageError> {
        if !claim.authorizes(&self.views) {
            return Err(StorageError::AssetAccessConflict);
        }
        if let Some(resolver) = &mut self.resolver {
            resolver.root.validity.check_write()?;
            if !resolver.writable {
                return Err(StorageError::AssetReadOnly);
            }
            let guard = resolver.root.state.try_write()?;
            let preflight = resolver.resolve_read_under_write(&guard)?;
            return Ok(AssetViewWrite {
                inner: AssetViewWriteInner::Resolved {
                    resolver,
                    guard,
                    preflight,
                },
                committed: false,
            });
        }
        match &mut self.inner {
            AssetStorageInner::Owned(Some(asset)) => Ok(AssetViewWrite {
                inner: AssetViewWriteInner::Direct(&mut **asset),
                committed: false,
            }),
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            AssetStorageInner::OwnedReadOnly(_) => Err(StorageError::OwnedFieldReadOnly),
            AssetStorageInner::BorrowedRef { .. } | AssetStorageInner::Source(_) => {
                Err(StorageError::AssetReadOnly)
            }
            AssetStorageInner::BorrowedMut { borrow, .. } => Ok(AssetViewWrite {
                inner: AssetViewWriteInner::Direct(borrow.get_mut()?),
                committed: false,
            }),
        }
    }

    /// Claim a zero-copy read view over this asset.
    ///
    /// Tracked wrappers coordinate through the world-owned resource state, so
    /// wrappers created by distinct access scopes still exclude a conflicting
    /// write view. Owned and untracked wrappers use their local counters.
    pub fn prepare_read_view(&self) -> Result<ReadViewClaim, StorageError> {
        if let Some(resolver) = &self.resolver {
            resolver.root.validity.check_read()?;
            let _resource_guard = resolver.root.state.try_read()?;
            if resolver
                .root
                .state
                .has_write_views(resolver.root.asset_id.into())
            {
                return Err(StorageError::AssetViewsLive);
            }
            return self
                .views
                .try_prepare_read()
                .ok_or(StorageError::AssetViewsLive);
        }
        self.views
            .try_prepare_read()
            .ok_or(StorageError::AssetViewsLive)
    }

    /// Claim a zero-copy write view over this asset.
    ///
    /// The resource-wide guard makes the cross-scope check and local counter
    /// claim one atomic transaction with respect to other Python resolvers.
    pub fn prepare_write_view(&self) -> Result<PendingViewClaim, StorageError> {
        if let Some(resolver) = &self.resolver {
            resolver.root.validity.check_write()?;
            if !resolver.writable {
                return Err(StorageError::AssetReadOnly);
            }
            let _resource_guard = resolver.root.state.try_write()?;
            if resolver.root.state.has_views(resolver.root.asset_id.into()) {
                return Err(StorageError::AssetViewsLive);
            }
            return self
                .views
                .try_prepare_write()
                .ok_or(StorageError::AssetViewsLive);
        }
        self.views
            .try_prepare_write()
            .ok_or(StorageError::AssetViewsLive)
    }

    /// Counters tracking live zero-copy NumPy views over this asset's data
    pub fn view_counters(&self) -> &ViewCounters {
        &self.views
    }

    /// The validity flag gating a borrowed asset, or `None` for an owned asset
    /// (which is always live). Used to build liveness probes for zero-copy
    /// bounded arrays over this asset's data.
    pub fn validity_flag(&self) -> Option<ValidityFlag> {
        match &self.inner {
            AssetStorageInner::Owned(_) | AssetStorageInner::OwnedReadOnly(_) => None,
            AssetStorageInner::BorrowedRef { borrow, .. } => Some(borrow.validity().clone()),
            AssetStorageInner::BorrowedMut { borrow, .. } => Some(borrow.validity().clone()),
            AssetStorageInner::Source(_) => None,
        }
    }

    /// Check if this storage contains an owned asset
    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        matches!(
            self.inner,
            AssetStorageInner::Owned(_) | AssetStorageInner::OwnedReadOnly(_)
        )
    }

    /// Check if this storage contains a borrowed asset
    #[allow(dead_code)]
    pub fn is_borrowed(&self) -> bool {
        matches!(
            self.inner,
            AssetStorageInner::BorrowedRef { .. }
                | AssetStorageInner::BorrowedMut { .. }
                | AssetStorageInner::Source(_)
        )
    }
}

impl<T: Asset> AssetStorage<T> {
    /// Convert storage to owned asset, consuming self
    ///
    /// # Errors
    /// Returns an error if storage contains a borrowed reference or if the asset
    /// was already consumed.
    pub fn into_owned(mut self) -> Result<T, StorageError> {
        self.take()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use bevy::{
        app::{App, TaskPoolPlugin},
        asset::{
            Asset, AssetApp, AssetEvent, AssetPlugin, Handle, UntypedAssetId,
            VisitAssetDependencies,
        },
        ecs::{component::Component, message::Messages, world::World},
        reflect::TypePath,
    };

    use super::*;
    use crate::{
        AccessMode, ComponentStorage, FieldStorage, ValidityFlag, ValidityGuard, ValueStorage,
    };

    #[test]
    fn cas_acquire_enforces_exclusion() {
        let c = ViewCounters::default();
        // First read succeeds; a second read coexists; a write is then rejected.
        let first = c.try_prepare_read().expect("first read");
        let second = c.try_prepare_read().expect("second read");
        assert!(!c.try_acquire_write());
        assert_eq!(c.write_count(), 0);
        drop((first, second));
        // A write succeeds when idle; a second write and any read are rejected.
        let write = c.try_prepare_write().expect("write");
        assert!(c.try_prepare_write().is_none());
        assert!(c.try_prepare_read().is_none());
        assert_eq!(c.read_count(), 0);
        assert_eq!(c.write_count(), 1);
        drop(write);
    }

    /// Build a real `Assets<T>` in a World and return the pieces a tracked
    /// borrowed wrapper needs.
    fn tracked_world() -> (Box<World>, AssetId<TestAsset>) {
        let mut world = Box::new(World::new());
        let mut assets = Assets::<TestAsset>::default();
        let handle = assets.add(TestAsset { value: 1 });
        let asset_id = handle.id();
        world.insert_resource(assets);
        (world, asset_id)
    }

    fn tracked_storage<A: Asset>(
        world: &mut World,
        asset_id: AssetId<A>,
        validity: &ValidityFlag,
        counter: &AssetBorrowCounter,
    ) -> AssetStorage<A> {
        let world_cell = world.as_unsafe_world_cell();
        let state = counter.scope().resource_state().clone();
        let _resource_guard = state.try_read().expect("initial read guard");
        // SAFETY: this helper has scheduler-equivalent exclusive access to
        // `world`, but intentionally derives only a shared initial pointer.
        let ptr: *const A = unsafe { world_cell.get_resource::<Assets<A>>() }
            .expect("Assets present")
            .get(asset_id)
            .expect("asset present");
        // SAFETY: the world outlives the storage and validity gates access.
        // Mutable provenance is re-derived only by the resolver.
        unsafe {
            AssetStorage::borrowed_mut_tracked(
                ptr,
                world_cell,
                asset_id,
                validity.with_access_mode(AccessMode::Write),
                counter.clone(),
            )
        }
        .expect("live tracked asset scope")
    }

    /// Writing through a wrapper after a second wrapper for the same asset has
    /// superseded its borrow chain must still land on the live asset.
    ///
    /// `Assets::get_mut` does not reject overlapping wrappers, so the first
    /// wrapper's captured pointer goes stale as soon as the second is taken.
    /// The change tracker re-derives on every mutable access to cover this;
    /// a refresh that only ran on the first write would leave this case broken.
    #[test]
    fn interleaved_wrappers_write_through_live_pointer() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new();
        let _guard = ValidityGuard::new(validity.clone());
        let registry = AssetAccessRegistry::default();
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<TestAsset>(),
            "TestAsset",
            validity.clone(),
            "system:test",
        ));

        let mut first = tracked_storage(&mut world, asset_id, &validity, &counter);
        first.as_mut().expect("first write").value = 10;

        // A second wrapper supersedes the first wrapper's borrow chain.
        let mut second = tracked_storage(&mut world, asset_id, &validity, &counter);
        second.as_mut().expect("second write").value = 20;

        // The first wrapper is used again: its original pointer is stale, so
        // this only lands correctly because as_mut re-derives.
        first.as_mut().expect("interleaved write").value = 30;

        let stored = world
            .resource::<Assets<TestAsset>>()
            .get(asset_id)
            .expect("asset present")
            .value;
        assert_eq!(stored, 30, "interleaved write must reach the live asset");
    }

    #[test]
    fn tracked_asset_write_marks_assets_resource_changed() {
        let (mut world, asset_id) = tracked_world();
        world.clear_trackers();
        assert!(!world.is_resource_changed::<Assets<TestAsset>>());

        let validity = ValidityFlag::new_write();
        let _guard = ValidityGuard::new(validity.clone());
        let counter = AssetBorrowCounter::default();
        let mut storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        storage.as_mut().expect("asset write").value = 2;
        drop(storage);

        assert!(world.is_resource_changed::<Assets<TestAsset>>());
    }

    /// Every mutable access re-derives, but only the first marks the asset, so
    /// Bevy still sees one `Modified` per wrapper.
    #[test]
    fn repeated_writes_mark_once_per_wrapper() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());

        let mut storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        storage.as_mut().expect("first").value = 1;
        storage.as_mut().expect("second").value = 2;
        storage.as_mut().expect("third").value = 3;

        assert_eq!(
            world
                .resource::<Assets<TestAsset>>()
                .get(asset_id)
                .expect("asset present")
                .value,
            3
        );
    }

    #[test]
    fn wrapper_creation_is_read_only_and_lazy() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let state = counter.scope().resource_state().clone();
        let before = state.epoch();

        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);

        assert_eq!(state.epoch(), before);
        assert_eq!(
            storage
                .as_ref()
                .expect("read through mutable wrapper")
                .value,
            1
        );
        assert_eq!(state.epoch(), before);
    }

    #[test]
    fn resource_guard_covers_sibling_asset_ids() {
        let mut world = Box::new(World::new());
        let mut assets = Assets::<TestAsset>::default();
        let first_id = assets.add(TestAsset { value: 1 }).id();
        let second_id = assets.add(TestAsset { value: 2 }).id();
        world.insert_resource(assets);
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let first = tracked_storage(&mut world, first_id, &validity, &counter);
        let mut second = tracked_storage(&mut world, second_id, &validity, &counter);

        let first_read = first.as_ref().expect("first reader");
        assert!(matches!(
            second.as_mut(),
            Err(StorageError::AssetAccessConflict)
        ));
        drop(first_read);
        second.as_mut().expect("writer after reader drops").value = 3;
    }

    #[test]
    fn view_transaction_rejects_a_claim_for_another_asset() {
        let mut world = Box::new(World::new());
        let mut assets = Assets::<TestAsset>::default();
        let first_id = assets.add(TestAsset { value: 1 }).id();
        let second_id = assets.add(TestAsset { value: 2 }).id();
        world.insert_resource(assets);
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let first = tracked_storage(&mut world, first_id, &validity, &counter);
        let mut second = tracked_storage(&mut world, second_id, &validity, &counter);
        let claim = first.prepare_write_view().expect("first asset claim");

        assert!(matches!(
            second.begin_write_view(&claim),
            Err(StorageError::AssetAccessConflict)
        ));
    }

    #[test]
    fn cloned_mutable_wrapper_is_read_only() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let mut cloned = storage.clone();

        assert_eq!(cloned.as_ref().expect("clone remains readable").value, 1);
        assert!(matches!(cloned.as_mut(), Err(StorageError::AssetReadOnly)));
    }

    #[test]
    fn nested_field_survives_parent_drop_and_writes_through() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let mut field: ValueStorage<i32> = storage
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("live field");

        drop(storage);
        *field.as_mut().expect("field remains writable") = 7;

        assert_eq!(*field.as_ref().expect("field remains readable"), 7);
        assert_eq!(
            world
                .resource::<Assets<TestAsset>>()
                .get(asset_id)
                .expect("asset present")
                .value,
            7
        );
    }

    #[test]
    fn nested_field_reapplies_path_after_root_write() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let mut field: ValueStorage<i32> = storage
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("live field");
        let mut sibling = tracked_storage(&mut world, asset_id, &validity, &counter);

        sibling.as_mut().expect("root write").value = 11;
        assert_eq!(*field.as_ref().expect("refreshed field read"), 11);
        *field.as_mut().expect("refreshed field write") = 13;

        assert_eq!(sibling.as_ref().expect("root read").value, 13);
    }

    #[test]
    fn enum_payload_path_revalidates_its_variant() {
        let mut world = Box::new(World::new());
        let mut assets = Assets::<VariantAsset>::default();
        let asset_id = assets
            .add(VariantAsset {
                value: TestVariant::First(3),
            })
            .id();
        world.insert_resource(assets);
        let validity = ValidityFlag::new_write();
        let registry = AssetAccessRegistry::default();
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<VariantAsset>(),
            "VariantAsset",
            validity.clone(),
            "system:test",
        ));
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let variant: ValueStorage<TestVariant> = storage
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("variant field");
        let mut payload: TestValueWrapper = variant
            .borrow_resolved_variant_as(
                "TestVariant.First",
                |value| match value {
                    TestVariant::First(payload) => Some(payload),
                    TestVariant::Second(_) => None,
                },
                |value| match value {
                    TestVariant::First(payload) => Some(payload),
                    TestVariant::Second(_) => None,
                },
            )
            .expect("variant payload");

        *payload.0.as_mut().expect("payload write") = 5;
        assert_eq!(*payload.0.as_ref().expect("payload read"), 5);

        let mut sibling = tracked_storage(&mut world, asset_id, &validity, &counter);
        sibling.as_mut().expect("variant replacement").value = TestVariant::Second(7);

        assert!(matches!(
            payload.0.as_ref(),
            Err(StorageError::VariantChanged("TestVariant.First"))
        ));
        assert!(matches!(
            payload.0.as_mut(),
            Err(StorageError::VariantChanged("TestVariant.First"))
        ));
        assert_eq!(
            world
                .resource::<Assets<VariantAsset>>()
                .get(asset_id)
                .expect("asset present")
                .value,
            TestVariant::Second(7)
        );
    }

    #[test]
    fn field_storage_enum_payload_path_revalidates_its_variant() {
        let mut world = Box::new(World::new());
        let mut assets = Assets::<VariantAsset>::default();
        let asset_id = assets
            .add(VariantAsset {
                value: TestVariant::First(3),
            })
            .id();
        world.insert_resource(assets);
        let validity = ValidityFlag::new_write();
        let registry = AssetAccessRegistry::default();
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<VariantAsset>(),
            "VariantAsset",
            validity.clone(),
            "system:test",
        ));
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let variant: FieldStorage<TestVariant> = storage
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("variant field");
        let mut payload: TestFieldWrapper = variant
            .borrow_resolved_variant_as(
                "TestVariant.First",
                |value| match value {
                    TestVariant::First(payload) => Some(payload),
                    TestVariant::Second(_) => None,
                },
                |value| match value {
                    TestVariant::First(payload) => Some(payload),
                    TestVariant::Second(_) => None,
                },
            )
            .expect("variant payload");

        *payload.0.as_mut().expect("payload write") = 5;
        let mut sibling = tracked_storage(&mut world, asset_id, &validity, &counter);
        sibling.as_mut().expect("variant replacement").value = TestVariant::Second(7);

        assert!(matches!(
            payload.0.as_ref(),
            Err(StorageError::VariantChanged("TestVariant.First"))
        ));
        assert!(matches!(
            payload.0.as_mut(),
            Err(StorageError::VariantChanged("TestVariant.First"))
        ));
    }

    #[test]
    fn failed_enum_payload_write_does_not_emit_modified() {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<VariantAsset>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<VariantAsset>>()
            .add(VariantAsset {
                value: TestVariant::First(3),
            });
        let asset_id = handle.id();
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<VariantAsset>>>()
            .clear();

        let validity = ValidityFlag::new_write();
        let registry = AssetAccessRegistry::default();
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<VariantAsset>(),
            "VariantAsset",
            validity.clone(),
            "system:test",
        ));
        let guard = ValidityGuard::new(validity.clone());
        let first = tracked_storage(app.world_mut(), asset_id, &validity, &counter);
        let variant: ValueStorage<TestVariant> = first
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("variant field");
        let mut payload: TestValueWrapper = variant
            .borrow_resolved_variant_as(
                "TestVariant.First",
                |value| match value {
                    TestVariant::First(payload) => Some(payload),
                    TestVariant::Second(_) => None,
                },
                |value| match value {
                    TestVariant::First(payload) => Some(payload),
                    TestVariant::Second(_) => None,
                },
            )
            .expect("variant payload");
        let mut second = tracked_storage(app.world_mut(), asset_id, &validity, &counter);

        second.as_mut().expect("variant replacement").value = TestVariant::Second(7);
        assert!(matches!(
            payload.0.as_mut(),
            Err(StorageError::VariantChanged("TestVariant.First"))
        ));

        drop(payload);
        drop(variant);
        drop(first);
        drop(second);
        drop(guard);
        app.update();

        let modified = app
            .world_mut()
            .resource_mut::<Messages<AssetEvent<VariantAsset>>>()
            .drain()
            .filter(|event| matches!(event, AssetEvent::Modified { id } if *id == asset_id))
            .count();
        assert_eq!(modified, 1);
        drop(handle);
    }

    #[test]
    fn nested_field_writes_emit_one_modified_event() {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<TestAsset>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<TestAsset>>()
            .add(TestAsset { value: 1 });
        let asset_id = handle.id();
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<TestAsset>>>()
            .clear();

        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(app.world_mut(), asset_id, &validity, &counter);
        let mut field: ValueStorage<i32> = storage
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("live field");

        *field.as_mut().expect("first write") = 7;
        *field.as_mut().expect("second write") = 9;
        drop(field);
        drop(storage);
        drop(guard);
        app.update();

        let modified = app
            .world_mut()
            .resource_mut::<Messages<AssetEvent<TestAsset>>>()
            .drain()
            .filter(|event| matches!(event, AssetEvent::Modified { id } if *id == asset_id))
            .count();
        assert_eq!(modified, 1);
        drop(handle);
    }

    #[test]
    fn cloned_nested_field_is_read_only() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let field: ValueStorage<i32> = storage
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("live field");
        let mut cloned = field.clone();

        assert_eq!(*cloned.as_ref().expect("clone remains readable"), 1);
        assert!(matches!(cloned.as_mut(), Err(StorageError::AssetReadOnly)));
    }

    #[test]
    fn cloning_an_expired_nested_field_remains_inert() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let field: ValueStorage<i32> = storage
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("live field");

        drop(guard);
        let cloned = field.clone();

        assert!(matches!(cloned.as_ref(), Err(StorageError::InvalidAccess)));
    }

    #[test]
    fn every_borrowable_storage_writes_through_an_asset_path() {
        let mut world = Box::new(World::new());
        let mut assets = Assets::<StructuredAsset>::default();
        let asset_id = assets
            .add(StructuredAsset {
                value: 1,
                field: "before".to_owned(),
                list: vec![1, 2],
                map: [("first".to_owned(), 4)].into(),
                component: NestedComponent { value: 3 },
            })
            .id();
        world.insert_resource(assets);
        let validity = ValidityFlag::new_write();
        let registry = AssetAccessRegistry::default();
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<StructuredAsset>(),
            "StructuredAsset",
            validity.clone(),
            "system:test",
        ));
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);

        let mut value: ValueStorage<i32> = storage
            .borrow_field(|asset| &asset.value, |asset| &mut asset.value)
            .expect("value field");
        let mut field: FieldStorage<String> = storage
            .borrow_field(|asset| &asset.field, |asset| &mut asset.field)
            .expect("owned field");
        let mut list: FieldStorage<Vec<i32>> = storage
            .borrow_field(|asset| &asset.list, |asset| &mut asset.list)
            .expect("list field");
        let mut map: FieldStorage<BTreeMap<String, i32>> = storage
            .borrow_field(|asset| &asset.map, |asset| &mut asset.map)
            .expect("map field");
        let mut component: ComponentStorage<NestedComponent> = storage
            .borrow_field(|asset| &asset.component, |asset| &mut asset.component)
            .expect("component field");
        let mut nested_value: TestValueWrapper = component
            .borrow_resolved_field_as(
                |component| &component.value,
                |component| &mut component.value,
            )
            .expect("nested component field");

        *value.as_mut().expect("value write") = 10;
        field.as_mut().expect("field write").push_str(" after");
        list.as_mut().expect("list write").push(3);
        map.as_mut()
            .expect("map write")
            .insert("second".to_owned(), 5);
        component.as_mut().expect("component write").value = 20;
        drop(component);
        *nested_value.0.as_mut().expect("nested component write") = 30;

        let stored = world
            .resource::<Assets<StructuredAsset>>()
            .get(asset_id)
            .expect("asset present");
        assert_eq!(stored.value, 10);
        assert_eq!(stored.field, "before after");
        assert_eq!(stored.list, [1, 2, 3]);
        assert_eq!(stored.component.value, 30);
    }

    #[test]
    fn indexed_and_keyed_paths_revalidate_after_structural_changes() {
        let mut world = Box::new(World::new());
        let mut assets = Assets::<StructuredAsset>::default();
        let asset_id = assets
            .add(StructuredAsset {
                value: 1,
                field: "field".to_owned(),
                list: vec![1, 2],
                map: [("first".to_owned(), 4)].into(),
                component: NestedComponent { value: 3 },
            })
            .id();
        world.insert_resource(assets);
        let validity = ValidityFlag::new_write();
        let registry = AssetAccessRegistry::default();
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<StructuredAsset>(),
            "StructuredAsset",
            validity.clone(),
            "system:test",
        ));
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let mut list: FieldStorage<Vec<i32>> = storage
            .borrow_field(|asset| &asset.list, |asset| &mut asset.list)
            .expect("list field");
        let mut map: FieldStorage<BTreeMap<String, i32>> = storage
            .borrow_field(|asset| &asset.map, |asset| &mut asset.map)
            .expect("map field");
        let mut indexed: TestFieldWrapper = list
            .borrow_resolved_index_as::<i32, FieldStorage<i32>, TestFieldWrapper>(
                1,
                |values, index| values.get(index),
                |values, index| values.get_mut(index),
            )
            .expect("indexed child");
        let keyed: TestFieldWrapper = map
            .borrow_resolved_key_as::<String, i32, FieldStorage<i32>, TestFieldWrapper>(
                "first".to_owned(),
                |values, key| values.get(key),
                |values, key| values.get_mut(key),
            )
            .expect("keyed child");

        *indexed.0.as_mut().expect("indexed write") = 8;
        let mut keyed = keyed;
        *keyed.0.as_mut().expect("keyed write") = 9;
        list.as_mut().expect("list insert").insert(0, 7);
        assert_eq!(*indexed.0.as_ref().expect("shifted index read"), 1);
        map.as_mut().expect("key remove").remove("first");
        assert!(matches!(
            keyed.0.as_ref(),
            Err(StorageError::KeyNotFound(_))
        ));
        list.as_mut().expect("list truncate").truncate(1);
        assert!(matches!(
            indexed.0.as_mut(),
            Err(StorageError::IndexOutOfRange)
        ));
    }

    #[test]
    fn failed_indexed_and_keyed_writes_do_not_emit_modified() {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<StructuredAsset>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<StructuredAsset>>()
            .add(StructuredAsset {
                value: 1,
                field: "field".to_owned(),
                list: vec![1],
                map: [("first".to_owned(), 4)].into(),
                component: NestedComponent { value: 3 },
            });
        let asset_id = handle.id();
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<StructuredAsset>>>()
            .clear();

        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let guard = ValidityGuard::new(validity.clone());
        let first = tracked_storage(app.world_mut(), asset_id, &validity, &counter);
        let list: FieldStorage<Vec<i32>> = first
            .borrow_field(|asset| &asset.list, |asset| &mut asset.list)
            .expect("list field");
        let map: FieldStorage<BTreeMap<String, i32>> = first
            .borrow_field(|asset| &asset.map, |asset| &mut asset.map)
            .expect("map field");
        let mut indexed: TestFieldWrapper = list
            .borrow_resolved_index_as::<i32, FieldStorage<i32>, TestFieldWrapper>(
                0,
                |values, index| values.get(index),
                |values, index| values.get_mut(index),
            )
            .expect("indexed child");
        let mut keyed: TestFieldWrapper = map
            .borrow_resolved_key_as::<String, i32, FieldStorage<i32>, TestFieldWrapper>(
                "first".to_owned(),
                |values, key| values.get(key),
                |values, key| values.get_mut(key),
            )
            .expect("keyed child");
        let mut second = tracked_storage(app.world_mut(), asset_id, &validity, &counter);

        second
            .as_mut()
            .expect("structural replacement")
            .list
            .clear();
        second.as_mut().expect("structural replacement").map.clear();
        assert!(matches!(
            indexed.0.as_mut(),
            Err(StorageError::IndexOutOfRange)
        ));
        assert!(matches!(
            keyed.0.as_mut(),
            Err(StorageError::KeyNotFound(_))
        ));

        drop(indexed);
        drop(keyed);
        drop(list);
        drop(map);
        drop(first);
        drop(second);
        drop(guard);
        app.update();

        let modified = app
            .world_mut()
            .resource_mut::<Messages<AssetEvent<StructuredAsset>>>()
            .drain()
            .filter(|event| matches!(event, AssetEvent::Modified { id } if *id == asset_id))
            .count();
        assert_eq!(modified, 1);
        drop(handle);
    }

    #[test]
    fn nested_asset_storage_revalidates_and_downgrades_clones() {
        let mut world = Box::new(World::new());
        let mut assets = Assets::<AssetContainer>::default();
        let asset_id = assets
            .add(AssetContainer {
                nested: NestedAsset { value: 3 },
            })
            .id();
        world.insert_resource(assets);
        let validity = ValidityFlag::new_write();
        let registry = AssetAccessRegistry::default();
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<AssetContainer>(),
            "AssetContainer",
            validity.clone(),
            "system:test",
        ));
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let mut nested = storage
            .borrow_asset_field(|asset| &asset.nested, |asset| &mut asset.nested)
            .expect("nested asset field");
        drop(storage);

        nested.as_mut().expect("nested write").value = 7;
        assert_eq!(nested.as_ref().expect("nested read").value, 7);

        let mut clone = nested.clone();
        assert_eq!(clone.as_ref().expect("clone read").value, 7);
        assert!(matches!(clone.as_mut(), Err(StorageError::AssetReadOnly)));
    }

    #[test]
    fn owned_nested_asset_storage_is_a_read_only_snapshot() {
        let storage = AssetStorage::owned(AssetContainer {
            nested: NestedAsset { value: 3 },
        });
        let mut nested = storage
            .borrow_asset_field(|asset| &asset.nested, |asset| &mut asset.nested)
            .expect("nested asset field");

        assert_eq!(nested.as_ref().expect("snapshot read").value, 3);
        assert!(matches!(
            nested.as_mut(),
            Err(StorageError::OwnedFieldReadOnly)
        ));
    }

    #[test]
    fn cloning_an_expired_tracked_wrapper_does_not_reactivate_its_lease() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new();
        let guard = ValidityGuard::new(validity.clone());
        let registry = AssetAccessRegistry::default();
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<TestAsset>(),
            "TestAsset",
            validity.clone(),
            "system:test",
        ));
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        assert_eq!(counter.active(), 1);

        drop(guard);
        let cloned = storage.clone();

        assert_eq!(counter.active(), 0);
        assert!(matches!(cloned.as_ref(), Err(StorageError::InvalidAccess)));
    }

    #[test]
    fn stale_cached_read_refreshes_after_sibling_write() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let first = tracked_storage(&mut world, asset_id, &validity, &counter);
        let mut second = tracked_storage(&mut world, asset_id, &validity, &counter);
        let first_epoch = first
            .resolver
            .as_ref()
            .expect("tracked resolver")
            .root
            .cached_epoch
            .load(Ordering::Acquire);

        second.as_mut().expect("sibling write").value = 9;
        let current_epoch = counter.scope().resource_state().epoch();
        assert_ne!(current_epoch, first_epoch);
        assert_eq!(first.as_ref().expect("refreshed read").value, 9);
        assert_eq!(
            first
                .resolver
                .as_ref()
                .expect("tracked resolver")
                .root
                .cached_epoch
                .load(Ordering::Acquire),
            current_epoch
        );
    }

    #[test]
    fn write_preflight_rejects_a_guard_from_another_world() {
        let (mut world, asset_id) = tracked_world();
        let validity = ValidityFlag::new_write();
        let counter = AssetBorrowCounter::default();
        let _guard = ValidityGuard::new(validity.clone());
        let storage = tracked_storage(&mut world, asset_id, &validity, &counter);
        let other_registry = AssetAccessRegistry::default();
        let other_state = other_registry.state_for(TypeId::of::<TestAsset>(), "TestAsset");
        let wrong_guard = other_state.try_write().expect("unrelated guard");

        assert!(matches!(
            storage
                .resolver
                .as_ref()
                .expect("tracked resolver")
                .resolve_read_under_write(&wrong_guard),
            Err(StorageError::AssetAccessConflict)
        ));
    }

    /// Two wrappers for one asset must share view counters, so a zero-copy view
    /// held through the first blocks a conflicting acquisition through the
    /// second. `AssetBorrowCounter::views_for` is what establishes that sharing.
    #[test]
    fn shared_counter_excludes_across_wrappers() {
        let counter = AssetBorrowCounter::default();
        let asset_id = Handle::<TestAsset>::default().untyped().id();

        // Two wrappers built from the SAME counter (the memoized path).
        let first = counter.views_for(asset_id);
        let second = counter.views_for(asset_id);

        let _read = first.try_prepare_read().expect("read claim");
        assert!(
            !second.try_acquire_write(),
            "a live read view obtained through one wrapper must block a write \
             acquired through another wrapper of the same asset"
        );
    }

    /// Pruning must never drop an entry a live wrapper still holds: doing so
    /// would hand the next wrapper for that asset a fresh, unshared pair of
    /// counters, which is exactly the exclusion hole
    /// `independent_counters_do_not_exclude` documents.
    #[test]
    fn registering_other_assets_keeps_a_held_entry() {
        let counter = AssetBorrowCounter::default();
        let held_id = test_id();

        let held = counter.views_for(held_id);
        let _read = held.try_prepare_read().expect("read claim");

        for other in distinct_ids(64) {
            counter.views_for(other);
        }

        assert!(
            !counter.views_for(held_id).try_acquire_write(),
            "the retained entry must still exclude, so it is the same pair of \
             atomics the live view is counted on"
        );
    }

    /// A view released after its wrapper is gone still holds one atomic, so the
    /// entry survives until nothing references it.
    #[test]
    fn an_entry_outlives_its_wrapper_while_a_view_holds_one_counter() {
        let counter = AssetBorrowCounter::default();
        let asset_id = test_id();

        let wrapper = counter.views_for(asset_id);
        let view_claim = wrapper.try_prepare_write().expect("write claim");
        drop(wrapper);

        // Enough registrations to force a sweep past the entry.
        for other in distinct_ids(crate::asset_access_registry::MIN_VIEW_SWEEP_SIZE + 1) {
            counter.views_for(other);
        }

        assert!(
            counter.views_for(asset_id).try_prepare_read().is_none(),
            "the write view is still live, so its entry must not have been pruned"
        );
        drop(view_claim);
    }

    /// Entries nothing holds are dropped, so a scope that outlives one system
    /// run does not accumulate one per asset it ever borrowed.
    #[test]
    fn unreferenced_entries_do_not_accumulate() {
        let counter = AssetBorrowCounter::default();

        for other in distinct_ids(4096) {
            // Each wrapper is dropped before the next asset registers.
            drop(counter.views_for(other));
        }

        let (entries, _) = counter.scope.resource_state().view_registry_metrics();
        assert!(
            entries <= crate::asset_access_registry::MIN_VIEW_SWEEP_SIZE,
            "registry grew to {} entries for 4096 dropped wrappers",
            entries
        );
    }

    /// Sweeping on every registration is quadratic exactly where nothing can be
    /// pruned, which is what `Assets::__iter__` does: it materializes a wrapper
    /// for every asset, so the whole registry stays live while it grows.
    #[test]
    fn sweeping_stays_amortized_when_every_entry_is_held() {
        let counter = AssetBorrowCounter::default();
        let assets = 4096;

        let held: Vec<ViewCounters> = distinct_ids(assets)
            .into_iter()
            .map(|id| counter.views_for(id))
            .collect();

        let (entries, sweeps) = counter.scope.resource_state().view_registry_metrics();
        assert_eq!(entries, assets, "every entry is still held");
        // The threshold doubles per sweep, so the count is logarithmic in the
        // asset count and the total scanned work stays linear.
        assert!(
            sweeps <= 16,
            "expected O(log n) sweeps for {assets} held entries, got {sweeps}",
        );
        drop(held);
    }

    /// One panic inside the registry must not permanently fail every later
    /// asset borrow.
    #[test]
    fn a_poisoned_registry_still_serves_counters() {
        let counter = AssetBorrowCounter::default();
        let asset_id = test_id();

        let poisoning = std::panic::catch_unwind({
            let counter = counter.clone();
            move || {
                counter.scope.resource_state().poison_view_registry();
            }
        });
        assert!(poisoning.is_err());

        assert!(counter.views_for(asset_id).try_prepare_read().is_some());
    }

    /// Distinct access scopes for one world and native asset type share the
    /// same per-ID view claims through `AssetResourceState`.
    #[test]
    fn distinct_scopes_in_one_world_exclude_same_asset_views() {
        let (mut world, asset_id) = tracked_world();
        let registry = AssetAccessRegistry::default();
        let validity_a = ValidityFlag::new_write();
        let validity_b = ValidityFlag::new_write();
        let separate_a = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<TestAsset>(),
            "TestAsset",
            validity_a.clone(),
            "system:first",
        ));
        let separate_b = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<TestAsset>(),
            "TestAsset",
            validity_b.clone(),
            "system:second",
        ));
        let _guard_a = ValidityGuard::new(validity_a.clone());
        let _guard_b = ValidityGuard::new(validity_b.clone());
        let first = tracked_storage(&mut world, asset_id, &validity_a, &separate_a);
        let second = tracked_storage(&mut world, asset_id, &validity_b, &separate_b);

        let read_claim = first.prepare_read_view().expect("read view claim");
        assert!(matches!(
            second.prepare_write_view(),
            Err(StorageError::AssetViewsLive)
        ));
        drop(read_claim);
    }

    /// An escaped view from an expired system scope is inert and must not
    /// deny access to the asset in later systems.
    #[test]
    fn expired_scope_view_claims_do_not_block_later_scopes() {
        let (mut world, asset_id) = tracked_world();
        let registry = AssetAccessRegistry::default();
        let validity_a = ValidityFlag::new_write();
        let validity_b = ValidityFlag::new_write();
        let separate_a = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<TestAsset>(),
            "TestAsset",
            validity_a.clone(),
            "system:first",
        ));
        let separate_b = AssetBorrowCounter::from_scope(registry.new_scope(
            TypeId::of::<TestAsset>(),
            "TestAsset",
            validity_b.clone(),
            "system:second",
        ));

        let (first, read_claim) = {
            let guard = ValidityGuard::new(validity_a.clone());
            let first = tracked_storage(&mut world, asset_id, &validity_a, &separate_a);
            let read_claim = first.prepare_read_view().expect("read view claim");
            drop(guard);
            (first, read_claim)
        };

        let _guard_b = ValidityGuard::new(validity_b.clone());
        let second = tracked_storage(&mut world, asset_id, &validity_b, &separate_b);
        let claim = second
            .prepare_write_view()
            .expect("an expired scope must not retain global view exclusion");
        drop(claim);
        drop(read_claim);
        drop(first);
    }

    #[derive(Clone, Debug, PartialEq, TypePath)]
    struct TestAsset {
        value: i32,
    }

    impl VisitAssetDependencies for TestAsset {
        fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
    }

    impl Asset for TestAsset {}

    #[derive(Clone, Debug, PartialEq, Component)]
    struct NestedComponent {
        value: i32,
    }

    struct TestValueWrapper(ValueStorage<i32>);

    impl crate::FromBorrowedStorage<ValueStorage<i32>> for TestValueWrapper {
        fn from_borrowed(storage: ValueStorage<i32>) -> Self {
            Self(storage)
        }
    }

    struct TestFieldWrapper(FieldStorage<i32>);

    impl crate::FromBorrowedStorage<FieldStorage<i32>> for TestFieldWrapper {
        fn from_borrowed(storage: FieldStorage<i32>) -> Self {
            Self(storage)
        }
    }

    #[derive(Clone, Debug, PartialEq, TypePath)]
    struct StructuredAsset {
        value: i32,
        field: String,
        list: Vec<i32>,
        map: BTreeMap<String, i32>,
        component: NestedComponent,
    }

    impl VisitAssetDependencies for StructuredAsset {
        fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
    }

    impl Asset for StructuredAsset {}

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum TestVariant {
        First(i32),
        Second(i32),
    }

    #[derive(Clone, Debug, PartialEq, TypePath)]
    struct VariantAsset {
        value: TestVariant,
    }

    impl VisitAssetDependencies for VariantAsset {
        fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
    }

    impl Asset for VariantAsset {}

    #[derive(Clone, Debug, PartialEq, TypePath)]
    struct NestedAsset {
        value: i32,
    }

    impl VisitAssetDependencies for NestedAsset {
        fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
    }

    impl Asset for NestedAsset {}

    #[derive(Clone, Debug, PartialEq, TypePath)]
    struct AssetContainer {
        nested: NestedAsset,
    }

    impl VisitAssetDependencies for AssetContainer {
        fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
    }

    impl Asset for AssetContainer {}

    /// Create a test asset ID for use in tests.
    fn test_id() -> UntypedAssetId {
        Handle::<TestAsset>::default().id().untyped()
    }

    /// `count` distinct asset ids, none of them [`test_id`].
    fn distinct_ids(count: usize) -> Vec<UntypedAssetId> {
        let mut assets = Assets::<TestAsset>::default();
        (0..count)
            .map(|value| {
                assets
                    .add(TestAsset {
                        value: value as i32,
                    })
                    .id()
                    .untyped()
            })
            .collect()
    }

    /// Equality is by value in every storage mode. Comparing borrowed wrappers
    /// by pointer made two equal assets unequal, and made an owned asset never
    /// equal to a borrowed one.
    #[test]
    fn equality_compares_values_across_storage_modes() {
        let flag = ValidityFlag::new_read();
        let _guard = ValidityGuard::new(flag.clone());
        let mode = flag.with_access_mode(AccessMode::Read);

        let first = TestAsset { value: 42 };
        let second = TestAsset { value: 42 };
        let third = TestAsset { value: 7 };

        // SAFETY: the assets outlive the borrows within this test scope.
        let (borrowed_first, borrowed_second, borrowed_third) = unsafe {
            (
                AssetStorage::borrowed_readonly(&first, mode.clone(), test_id()),
                AssetStorage::borrowed_readonly(&second, mode.clone(), test_id()),
                AssetStorage::borrowed_readonly(&third, mode.clone(), test_id()),
            )
        };
        let owned = AssetStorage::owned(TestAsset { value: 42 });

        assert_eq!(borrowed_first, borrowed_second);
        assert_eq!(owned, borrowed_first);
        assert_ne!(borrowed_first, borrowed_third);
        assert_ne!(owned, borrowed_third);
    }

    /// An unreadable wrapper still equals itself, so `x == x` holds after the
    /// borrow expires or the asset is consumed.
    #[test]
    fn equality_falls_back_to_identity_when_unreadable() {
        // Never activated, so the borrow behaves like one whose system ended.
        let flag = ValidityFlag::new();
        let mode = flag.with_access_mode(AccessMode::Read);
        let asset = TestAsset { value: 42 };
        // SAFETY: the asset outlives the borrows within this test scope.
        let expired = unsafe { AssetStorage::borrowed_readonly(&asset, mode, test_id()) };

        assert!(expired.as_ref().is_err());
        assert_eq!(expired, expired.clone());

        let mut consumed = AssetStorage::owned(TestAsset { value: 1 });
        consumed.take().expect("first take");
        let mut other_consumed = AssetStorage::owned(TestAsset { value: 2 });
        other_consumed.take().expect("first take");
        assert_eq!(consumed, other_consumed);
    }

    #[test]
    fn test_owned_storage() {
        let storage = AssetStorage::owned(TestAsset { value: 42 });
        assert!(storage.is_owned());
        assert!(!storage.is_borrowed());
        assert_eq!(storage.as_ref().unwrap().value, 42);
    }

    #[test]
    fn test_owned_mutation() {
        let mut storage = AssetStorage::owned(TestAsset { value: 42 });
        storage.as_mut().unwrap().value = 100;
        assert_eq!(storage.as_ref().unwrap().value, 100);
    }

    #[test]
    fn test_borrowed_readonly_storage() {
        let asset = TestAsset { value: 42 };
        let validity = ValidityFlag::new_read();
        let id = test_id();

        let storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                id,
            )
        };

        assert!(!storage.is_owned());
        assert!(storage.is_borrowed());

        // Activate validity guard to allow access
        let _guard = ValidityGuard::new(validity.clone());
        assert_eq!(storage.as_ref().unwrap().value, 42);
    }

    #[test]
    fn test_borrowed_readonly_mutation_fails() {
        let asset = TestAsset { value: 42 };
        let validity = ValidityFlag::new_read();
        let id = test_id();

        let mut storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                id,
            )
        };

        // Activate validity guard
        let _guard = ValidityGuard::new(validity.clone());

        // Read should work
        assert!(storage.as_ref().is_ok());

        // Write should fail - borrowed as read-only
        assert!(storage.as_mut().is_err());
    }

    #[test]
    fn test_borrowed_mut_storage() {
        let mut asset = TestAsset { value: 42 };
        let validity = ValidityFlag::new_write();
        let id = test_id();

        let mut storage = unsafe {
            AssetStorage::borrowed_mut(
                &mut asset as *mut TestAsset,
                validity.with_access_mode(AccessMode::Write),
                id,
            )
        };

        assert!(!storage.is_owned());
        assert!(storage.is_borrowed());

        // Activate validity guard to allow access
        let _guard = ValidityGuard::new(validity.clone());
        storage.as_mut().unwrap().value = 100;
        assert_eq!(asset.value, 100);
        assert_eq!(storage.as_ref().unwrap().value, 100);
    }

    #[test]
    fn test_validity_enforcement() {
        let asset = TestAsset { value: 42 };
        let validity = ValidityFlag::new_read();
        let id = test_id();

        let storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                id,
            )
        };

        // Should work while valid (after guard activation)
        {
            let _guard = ValidityGuard::new(validity.clone());
            assert!(storage.as_ref().is_ok());
        }

        // Should fail when invalid (guard dropped)
        assert!(storage.as_ref().is_err());
    }

    #[test]
    fn test_into_owned_from_owned() {
        let storage = AssetStorage::owned(TestAsset { value: 42 });
        let asset = storage.into_owned().unwrap();
        assert_eq!(asset.value, 42);
    }

    #[test]
    fn test_into_owned_from_borrowed_fails() {
        let asset = TestAsset { value: 42 };
        let validity = ValidityFlag::new_read();
        let id = test_id();

        let storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                id,
            )
        };

        // Borrowed storage cannot be converted to owned
        assert!(storage.into_owned().is_err());
    }

    #[test]
    fn test_access_after_take_fails() {
        let mut storage = AssetStorage::owned(TestAsset { value: 42 });
        let _ = storage.take().unwrap();

        assert!(matches!(storage.as_ref(), Err(StorageError::AssetConsumed)));
        assert!(matches!(storage.as_mut(), Err(StorageError::AssetConsumed)));
    }

    #[test]
    fn test_double_take_fails() {
        let mut storage = AssetStorage::owned(TestAsset { value: 42 });
        assert!(storage.take().is_ok());
        assert!(matches!(storage.take(), Err(StorageError::AssetConsumed)));
    }

    #[test]
    fn test_take_on_borrowed_readonly_fails() {
        let asset = TestAsset { value: 42 };
        let validity = ValidityFlag::new_read();
        let id = test_id();

        let mut storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                id,
            )
        };

        assert!(matches!(storage.take(), Err(StorageError::AssetBorrowed)));
    }

    #[test]
    fn test_take_on_borrowed_mut_fails() {
        let mut asset = TestAsset { value: 42 };
        let validity = ValidityFlag::new_write();
        let id = test_id();

        let mut storage = unsafe {
            AssetStorage::borrowed_mut(
                &mut asset as *mut TestAsset,
                validity.with_access_mode(AccessMode::Write),
                id,
            )
        };

        assert!(matches!(storage.take(), Err(StorageError::AssetBorrowed)));
    }

    /// Live view counters gate reads against write views and gate mutation or
    /// consumption against any view.
    #[test]
    fn view_counters_gate_access() {
        let mut storage = AssetStorage::owned(TestAsset { value: 1 });

        let read_claim = storage.prepare_read_view().expect("read claim");
        assert!(storage.as_ref().is_ok());
        assert!(matches!(
            storage.as_mut(),
            Err(StorageError::AssetViewsLive)
        ));
        assert!(matches!(storage.take(), Err(StorageError::AssetViewsLive)));
        drop(read_claim);

        let write_claim = storage
            .view_counters()
            .try_prepare_write()
            .expect("write claim");
        assert!(matches!(
            storage.as_ref(),
            Err(StorageError::AssetViewsLive)
        ));
        assert!(matches!(
            storage.as_mut(),
            Err(StorageError::AssetViewsLive)
        ));
        drop(write_claim);

        assert!(storage.as_ref().is_ok());
        assert!(storage.as_mut().is_ok());
        assert!(storage.take().is_ok());
    }
}
