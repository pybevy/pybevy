use std::{
    any::TypeId,
    collections::HashMap,
    mem,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
};

use bevy::{asset::UntypedAssetId, ecs::world::World, prelude::Resource};

use crate::{AccessMode, ValidityFlag, validity_guard::InvalidationObserver};

const WRITE_GATE: usize = 1 << (usize::BITS - 1);
const READER_MASK: usize = WRITE_GATE - 1;
pub(crate) const MIN_VIEW_SWEEP_SIZE: usize = 64;
const VIEW_SWEEP_GROWTH: usize = 4;
const VIEW_PENDING: u8 = 0;
const VIEW_READY: u8 = 1;
const VIEW_CLOSED: u8 = 2;

#[derive(Debug, Clone, Default)]
pub struct ViewCounters {
    gate: Arc<AtomicUsize>,
}

/// Linear ownership of one acquired read-view claim.
#[derive(Debug)]
pub struct ReadViewClaim {
    gate: Arc<AtomicUsize>,
    released: AtomicBool,
}

impl ReadViewClaim {
    pub fn release(&self) {
        if !self.released.swap(true, Ordering::AcqRel) {
            let previous = self.gate.fetch_sub(1, Ordering::AcqRel);
            debug_assert!(previous & READER_MASK > 0 && previous & WRITE_GATE == 0);
        }
    }
}

impl Drop for ReadViewClaim {
    fn drop(&mut self) {
        self.release();
    }
}

/// Linear ownership of one acquired write-view counter.
#[derive(Debug)]
pub struct PendingViewClaim {
    gate: Arc<AtomicUsize>,
    state: AtomicU8,
}

impl PendingViewClaim {
    fn pending(gate: Arc<AtomicUsize>) -> Self {
        Self {
            gate,
            state: AtomicU8::new(VIEW_PENDING),
        }
    }

    pub fn authorizes(&self, views: &ViewCounters) -> bool {
        self.state.load(Ordering::Acquire) == VIEW_PENDING && Arc::ptr_eq(&self.gate, &views.gate)
    }

    pub fn commit(&self) {
        let result = self.state.compare_exchange(
            VIEW_PENDING,
            VIEW_READY,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        assert!(result.is_ok(), "only a pending view claim can be committed");
    }

    pub fn release(&self) {
        if self.state.swap(VIEW_CLOSED, Ordering::AcqRel) != VIEW_CLOSED {
            let previous = self.gate.swap(0, Ordering::AcqRel);
            debug_assert_eq!(previous, WRITE_GATE);
        }
    }
}

impl Drop for PendingViewClaim {
    fn drop(&mut self) {
        self.release();
    }
}

impl ViewCounters {
    fn counts(&self) -> usize {
        self.gate.load(Ordering::Acquire)
    }

    pub fn read_count(&self) -> usize {
        self.counts() & READER_MASK
    }

    pub fn write_count(&self) -> usize {
        usize::from(self.counts() & WRITE_GATE != 0)
    }

    pub fn check_no_write_views(&self) -> Result<(), crate::StorageError> {
        if self.write_count() > 0 {
            return Err(crate::StorageError::AssetViewsLive);
        }
        Ok(())
    }

    pub fn check_no_views(&self) -> Result<(), crate::StorageError> {
        if self.counts() != 0 {
            return Err(crate::StorageError::AssetViewsLive);
        }
        Ok(())
    }

    pub fn try_prepare_read(&self) -> Option<ReadViewClaim> {
        let mut current = self.gate.load(Ordering::Acquire);
        loop {
            if current & WRITE_GATE != 0 || current & READER_MASK == READER_MASK {
                return None;
            }
            match self.gate.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(ReadViewClaim {
                        gate: self.gate.clone(),
                        released: AtomicBool::new(false),
                    });
                }
                Err(actual) => current = actual,
            }
        }
    }

    pub fn try_acquire_write(&self) -> bool {
        self.gate
            .compare_exchange(0, WRITE_GATE, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn try_prepare_write(&self) -> Option<PendingViewClaim> {
        self.try_acquire_write()
            .then(|| PendingViewClaim::pending(self.gate.clone()))
    }

    fn is_referenced(&self) -> bool {
        Arc::strong_count(&self.gate) > 1 || self.counts() != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ViewKey {
    asset_id: UntypedAssetId,
    scope_id: u64,
}

#[derive(Debug)]
struct ViewEntry {
    counters: ViewCounters,
    scope: Weak<AssetAccessScopeInner>,
}

#[derive(Debug)]
struct ViewRegistry {
    entries: HashMap<ViewKey, ViewEntry>,
    sweep_at: usize,
    sweeps: usize,
}

impl Default for ViewRegistry {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            sweep_at: MIN_VIEW_SWEEP_SIZE,
            sweeps: 0,
        }
    }
}

#[derive(Debug)]
pub struct AssetResourceReadGuard {
    state: Arc<AssetResourceState>,
}

impl Drop for AssetResourceReadGuard {
    fn drop(&mut self) {
        let previous = self.state.gate.fetch_sub(1, Ordering::Release);
        debug_assert!(previous & READER_MASK > 0 && previous & WRITE_GATE == 0);
    }
}

#[derive(Debug)]
pub struct AssetResourceWriteGuard {
    state: Arc<AssetResourceState>,
}

impl AssetResourceWriteGuard {
    pub fn authorizes(&self, state: &Arc<AssetResourceState>) -> bool {
        Arc::ptr_eq(&self.state, state)
    }
}

impl Drop for AssetResourceWriteGuard {
    fn drop(&mut self) {
        let previous = self.state.gate.swap(0, Ordering::Release);
        debug_assert_eq!(previous, WRITE_GATE);
    }
}

#[derive(Debug, Clone)]
pub struct ActiveAssetAccess {
    pub asset_name: Arc<str>,
    pub origin: Arc<str>,
    pub asset_id: UntypedAssetId,
}

#[derive(Debug, Default)]
struct ScopeCounts {
    drained: bool,
    total: usize,
    next_asset_key: u64,
    by_asset: HashMap<UntypedAssetId, (u64, usize)>,
    by_key: HashMap<u64, UntypedAssetId>,
}

#[derive(Debug)]
struct AssetAccessScopeInner {
    id: u64,
    origin: Arc<str>,
    validity: ValidityFlag,
    counts: Mutex<ScopeCounts>,
    resource_state: Arc<AssetResourceState>,
    registry_active: Arc<AtomicUsize>,
}

impl AssetAccessScopeInner {
    fn acquire(&self, asset_id: UntypedAssetId) -> Option<u64> {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if counts.drained {
            return None;
        }
        let asset_key = if let Some((key, count)) = counts.by_asset.get_mut(&asset_id) {
            *count += 1;
            *key
        } else {
            let key = counts.next_asset_key;
            counts.next_asset_key = counts
                .next_asset_key
                .checked_add(1)
                .expect("asset scope key space exhausted");
            counts.by_asset.insert(asset_id, (key, 1));
            counts.by_key.insert(key, asset_id);
            key
        };
        counts.total += 1;
        self.resource_state.active.fetch_add(1, Ordering::AcqRel);
        self.registry_active.fetch_add(1, Ordering::AcqRel);
        Some(asset_key)
    }

    fn acquire_existing(&self, asset_key: u64) -> bool {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if counts.drained {
            return false;
        }
        let asset_id = *counts
            .by_key
            .get(&asset_key)
            .expect("asset scope key is not registered");
        counts
            .by_asset
            .get_mut(&asset_id)
            .expect("asset scope key has no asset entry")
            .1 += 1;
        counts.total += 1;
        self.resource_state.active.fetch_add(1, Ordering::AcqRel);
        self.registry_active.fetch_add(1, Ordering::AcqRel);
        true
    }

    fn release(&self, asset_key: u64) {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if counts.drained {
            return;
        }

        counts.total = counts
            .total
            .checked_sub(1)
            .expect("asset lease count underflow");
        let asset_id = *counts
            .by_key
            .get(&asset_key)
            .expect("asset scope key is not registered");
        let remove = {
            let (_, count) = counts
                .by_asset
                .get_mut(&asset_id)
                .expect("asset lease missing from its access scope");
            *count = count
                .checked_sub(1)
                .expect("per-asset lease count underflow");
            *count == 0
        };
        if remove {
            counts.by_asset.remove(&asset_id);
            counts.by_key.remove(&asset_key);
        }
        self.resource_state.active.fetch_sub(1, Ordering::AcqRel);
        self.registry_active.fetch_sub(1, Ordering::AcqRel);
    }

    fn drain(&self) {
        let drained = {
            let mut counts = self
                .counts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if counts.drained {
                return;
            }
            counts.drained = true;
            counts.by_asset.clear();
            counts.by_key.clear();
            mem::take(&mut counts.total)
        };
        self.resource_state
            .active
            .fetch_sub(drained, Ordering::AcqRel);
        self.registry_active.fetch_sub(drained, Ordering::AcqRel);
    }

    fn is_valid(&self) -> bool {
        !matches!(self.validity.get_mode(), AccessMode::Invalid)
    }

    fn first_asset(&self) -> Option<UntypedAssetId> {
        let counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if counts.drained {
            None
        } else {
            counts.by_asset.keys().next().copied()
        }
    }
}

impl InvalidationObserver for AssetAccessScopeInner {
    fn invalidated(&self) {
        self.drain();
    }
}

/// One validity-bound access scope for a native `Assets<T>` resource.
#[derive(Debug, Clone)]
pub struct AssetAccessScope {
    inner: Arc<AssetAccessScopeInner>,
}

impl AssetAccessScope {
    fn new(
        id: u64,
        origin: Arc<str>,
        validity: ValidityFlag,
        resource_state: Arc<AssetResourceState>,
        registry_active: Arc<AtomicUsize>,
    ) -> Self {
        let inner = Arc::new(AssetAccessScopeInner {
            id,
            origin,
            validity: validity.clone(),
            counts: Mutex::new(ScopeCounts::default()),
            resource_state,
            registry_active,
        });
        let observer: Arc<dyn InvalidationObserver> = inner.clone();
        validity.observe_invalidation(&observer);
        Self { inner }
    }

    pub(crate) fn acquire(&self, asset_id: UntypedAssetId) -> Option<u64> {
        self.inner.acquire(asset_id)
    }

    pub(crate) fn acquire_existing(&self, asset_key: u64) -> bool {
        self.inner.acquire_existing(asset_key)
    }

    pub(crate) fn release(&self, asset_key: u64) {
        self.inner.release(asset_key);
    }

    pub fn active(&self) -> usize {
        let counts = self
            .inner
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if counts.drained { 0 } else { counts.total }
    }

    pub fn origin(&self) -> &str {
        &self.inner.origin
    }

    pub fn resource_state(&self) -> &Arc<AssetResourceState> {
        &self.inner.resource_state
    }
}

/// Persistent state shared by every access path for one native asset type.
#[derive(Debug)]
pub struct AssetResourceState {
    type_id: TypeId,
    asset_name: Arc<str>,
    active: AtomicUsize,
    scopes: Mutex<HashMap<u64, Weak<AssetAccessScopeInner>>>,
    views: Mutex<ViewRegistry>,
    gate: AtomicUsize,
    epoch: AtomicU64,
}

impl AssetResourceState {
    fn new(type_id: TypeId, asset_name: Arc<str>) -> Self {
        Self {
            type_id,
            asset_name,
            active: AtomicUsize::new(0),
            scopes: Mutex::new(HashMap::new()),
            views: Mutex::new(ViewRegistry::default()),
            gate: AtomicUsize::new(0),
            epoch: AtomicU64::new(0),
        }
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub fn asset_name(&self) -> &str {
        &self.asset_name
    }

    pub fn active(&self) -> usize {
        let active = self.active.load(Ordering::Acquire);
        if active == 0 {
            return 0;
        }
        self.prune_scopes();
        self.active.load(Ordering::Acquire)
    }

    pub fn views_for(
        self: &Arc<Self>,
        asset_id: UntypedAssetId,
        scope: &AssetAccessScope,
    ) -> ViewCounters {
        let mut registry = self
            .views
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = ViewKey {
            asset_id,
            scope_id: scope.inner.id,
        };
        if registry.entries.len() >= registry.sweep_at && !registry.entries.contains_key(&key) {
            registry.entries.retain(|_, entry| {
                entry
                    .scope
                    .upgrade()
                    .is_some_and(|scope| scope.is_valid() && entry.counters.is_referenced())
            });
            registry.sweep_at = registry
                .entries
                .len()
                .saturating_mul(VIEW_SWEEP_GROWTH)
                .max(MIN_VIEW_SWEEP_SIZE);
            registry.sweeps += 1;
        }
        let entry = registry.entries.entry(key).or_insert_with(|| ViewEntry {
            counters: ViewCounters::default(),
            scope: Arc::downgrade(&scope.inner),
        });
        entry.counters.clone()
    }

    pub(crate) fn has_write_views(&self, asset_id: UntypedAssetId) -> bool {
        self.has_matching_views(asset_id, false)
    }

    pub(crate) fn has_views(&self, asset_id: UntypedAssetId) -> bool {
        self.has_matching_views(asset_id, true)
    }

    fn has_matching_views(&self, asset_id: UntypedAssetId, include_reads: bool) -> bool {
        let mut registry = self
            .views
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.entries.retain(|_, entry| {
            entry
                .scope
                .upgrade()
                .is_some_and(|scope| scope.is_valid() && entry.counters.is_referenced())
        });
        registry.entries.iter().any(|(key, entry)| {
            key.asset_id == asset_id
                && (entry.counters.write_count() > 0
                    || include_reads && entry.counters.read_count() > 0)
        })
    }

    pub fn try_read(self: &Arc<Self>) -> Result<AssetResourceReadGuard, crate::StorageError> {
        let mut current = self.gate.load(Ordering::Acquire);
        loop {
            if current & WRITE_GATE != 0 || current & READER_MASK == READER_MASK {
                return Err(crate::StorageError::AssetAccessConflict);
            }
            match self.gate.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(AssetResourceReadGuard {
                        state: self.clone(),
                    });
                }
                Err(actual) => current = actual,
            }
        }
    }

    pub fn try_write(self: &Arc<Self>) -> Result<AssetResourceWriteGuard, crate::StorageError> {
        self.gate
            .compare_exchange(0, WRITE_GATE, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| crate::StorageError::AssetAccessConflict)?;
        Ok(AssetResourceWriteGuard {
            state: self.clone(),
        })
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    pub fn advance_epoch(&self) -> u64 {
        match self
            .epoch
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current != u64::MAX).then_some(current.saturating_add(1))
            }) {
            Ok(previous) => previous.saturating_add(1),
            Err(terminal) => terminal,
        }
    }

    pub fn epoch_is_cacheable(epoch: u64) -> bool {
        epoch != u64::MAX
    }

    #[cfg(test)]
    pub(crate) fn view_registry_metrics(&self) -> (usize, usize) {
        let registry = self
            .views
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (registry.entries.len(), registry.sweeps)
    }

    #[cfg(test)]
    pub(crate) fn poison_view_registry(&self) {
        let _guard = self.views.lock().expect("first lock succeeds");
        panic!("poison the registry");
    }

    #[cfg(test)]
    fn set_epoch(&self, epoch: u64) {
        self.epoch.store(epoch, Ordering::Release);
    }

    fn register_scope(&self, scope: &AssetAccessScope) {
        self.scopes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(scope.inner.id, Arc::downgrade(&scope.inner));
    }

    fn prune_scopes(&self) {
        self.scopes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|_, scope| {
                let Some(scope) = scope.upgrade() else {
                    return false;
                };
                if !scope.is_valid() {
                    scope.drain();
                    return false;
                }
                true
            });
    }

    fn first_active(&self) -> Option<ActiveAssetAccess> {
        self.prune_scopes();
        let scopes = self
            .scopes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        scopes
            .iter()
            .filter_map(|(scope_id, scope)| {
                let scope = scope.upgrade()?;
                let asset_id = scope.first_asset()?;
                Some((*scope_id, scope.origin.clone(), asset_id))
            })
            .min_by_key(|(scope_id, _, _)| *scope_id)
            .map(|(_, origin, asset_id)| ActiveAssetAccess {
                asset_name: self.asset_name.clone(),
                origin,
                asset_id,
            })
    }
}

/// World-owned registry for all native asset access scopes.
#[derive(Resource, Debug)]
pub struct AssetAccessRegistry {
    states: Mutex<HashMap<TypeId, Arc<AssetResourceState>>>,
    next_scope_id: AtomicU64,
    active: Arc<AtomicUsize>,
}

impl Default for AssetAccessRegistry {
    fn default() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            next_scope_id: AtomicU64::new(1),
            active: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl AssetAccessRegistry {
    pub fn state_for(
        &self,
        type_id: TypeId,
        asset_name: impl Into<Arc<str>>,
    ) -> Arc<AssetResourceState> {
        self.states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(type_id)
            .or_insert_with(|| Arc::new(AssetResourceState::new(type_id, asset_name.into())))
            .clone()
    }

    pub fn new_scope(
        &self,
        type_id: TypeId,
        asset_name: impl Into<Arc<str>>,
        validity: ValidityFlag,
        origin: impl Into<Arc<str>>,
    ) -> AssetAccessScope {
        let state = self.state_for(type_id, asset_name);
        let scope = AssetAccessScope::new(
            self.next_scope_id.fetch_add(1, Ordering::Relaxed),
            origin.into(),
            validity,
            state.clone(),
            self.active.clone(),
        );
        state.register_scope(&scope);
        scope
    }

    /// Number of valid per-asset wrapper leases in the world.
    ///
    /// The zero case is a single atomic load. A nonzero count performs the
    /// diagnostic sweep that also repairs any missed invalidation callback.
    pub fn active(&self) -> usize {
        let active = self.active.load(Ordering::Acquire);
        if active == 0 {
            return 0;
        }
        for state in self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
        {
            state.prune_scopes();
        }
        self.active.load(Ordering::Acquire)
    }

    pub fn first_active(&self) -> Option<ActiveAssetAccess> {
        if self.active.load(Ordering::Acquire) == 0 {
            return None;
        }
        let states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        states
            .values()
            .filter_map(|state| state.first_active())
            .min_by(|left, right| left.asset_name.cmp(&right.asset_name))
    }

    #[cfg(test)]
    fn scope_count(&self, type_id: TypeId) -> usize {
        let states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(state) = states.get(&type_id) else {
            return 0;
        };
        state.prune_scopes();
        state
            .scopes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

pub fn ensure_asset_access_registry(world: &mut World) {
    world.init_resource::<AssetAccessRegistry>();
}

#[cfg(test)]
mod tests {
    use std::{sync::Barrier, thread};

    use bevy::{
        asset::{Asset, Handle},
        reflect::TypePath,
    };

    use super::*;
    use crate::ValidityGuard;

    struct MeshAsset;
    struct ImageAsset;

    fn test_id() -> UntypedAssetId {
        #[derive(Asset, TypePath)]
        struct TestAsset;

        Handle::<TestAsset>::default().id().untyped()
    }

    #[test]
    fn native_type_identity_is_shared_across_scopes() {
        let registry = AssetAccessRegistry::default();
        let first = registry.new_scope(
            TypeId::of::<MeshAsset>(),
            "Mesh",
            ValidityFlag::new_write(),
            "system:first",
        );
        let second = registry.new_scope(
            TypeId::of::<MeshAsset>(),
            "Mesh",
            ValidityFlag::new_write(),
            "World",
        );
        let other = registry.new_scope(
            TypeId::of::<ImageAsset>(),
            "Image",
            ValidityFlag::new_write(),
            "control",
        );

        assert!(Arc::ptr_eq(first.resource_state(), second.resource_state()));
        assert!(!Arc::ptr_eq(first.resource_state(), other.resource_state()));
    }

    #[test]
    fn racing_read_and_write_claims_cannot_both_abort() {
        for _ in 0..128 {
            let counters = ViewCounters::default();
            let start = Arc::new(Barrier::new(3));
            let finish = Arc::new(Barrier::new(2));

            let read = {
                let counters = counters.clone();
                let start = start.clone();
                let finish = finish.clone();
                thread::spawn(move || {
                    start.wait();
                    let claim = counters.try_prepare_read();
                    finish.wait();
                    claim.is_some()
                })
            };
            let write = {
                let counters = counters.clone();
                let start = start.clone();
                let finish = finish.clone();
                thread::spawn(move || {
                    start.wait();
                    let claim = counters.try_prepare_write();
                    finish.wait();
                    claim.is_some()
                })
            };

            start.wait();
            let read = read.join().expect("read claimant");
            let write = write.join().expect("write claimant");
            assert_ne!((read, write), (false, false));
            assert_ne!((read, write), (true, true));
        }
    }

    #[test]
    fn invalidation_drains_each_scope_exactly_once() {
        let registry = AssetAccessRegistry::default();
        let validity = ValidityFlag::new();
        let guard = ValidityGuard::new(validity.clone());
        let scope =
            registry.new_scope(TypeId::of::<MeshAsset>(), "Mesh", validity, "system:update");
        let first = scope.acquire(test_id()).expect("live scope");
        let second = scope.acquire(test_id()).expect("live scope");
        assert_eq!(registry.active(), 2);

        drop(guard);
        assert_eq!(registry.active(), 0);
        assert_eq!(scope.active(), 0);
        scope.release(first);
        scope.release(second);
        assert_eq!(registry.active(), 0);
    }

    #[test]
    fn expired_scope_rejects_new_leases_without_panicking() {
        let registry = AssetAccessRegistry::default();
        let validity = ValidityFlag::new_write();
        let scope = registry.new_scope(
            TypeId::of::<MeshAsset>(),
            "Mesh",
            validity.clone(),
            "system:update",
        );

        validity.set_invalid();

        assert_eq!(scope.acquire(test_id()), None);
        assert_eq!(registry.active(), 0);
    }

    #[test]
    fn bare_scope_is_not_an_active_asset_token() {
        let registry = AssetAccessRegistry::default();
        let scope = registry.new_scope(
            TypeId::of::<MeshAsset>(),
            "Mesh",
            ValidityFlag::new_write(),
            "World",
        );

        assert_eq!(scope.active(), 0);
        assert_eq!(registry.active(), 0);
    }

    #[test]
    fn active_diagnostic_reports_type_origin_and_asset() {
        let registry = AssetAccessRegistry::default();
        let scope = registry.new_scope(
            TypeId::of::<MeshAsset>(),
            "Mesh",
            ValidityFlag::new_write(),
            "system:update",
        );
        let asset_id = test_id();
        let key = scope.acquire(asset_id).expect("live scope");

        let active = registry.first_active().expect("active lease is reported");
        assert_eq!(&*active.asset_name, "Mesh");
        assert_eq!(&*active.origin, "system:update");
        assert_eq!(active.asset_id, asset_id);
        scope.release(key);
        assert!(registry.first_active().is_none());
    }

    #[test]
    fn dead_scopes_are_pruned_without_retaining_history() {
        let registry = AssetAccessRegistry::default();
        let type_id = TypeId::of::<MeshAsset>();
        for _ in 0..128 {
            drop(registry.new_scope(type_id, "Mesh", ValidityFlag::new_write(), "request"));
        }

        assert_eq!(registry.scope_count(type_id), 0);
    }

    #[test]
    fn ensure_initializes_bare_worlds() {
        let mut world = World::new();
        ensure_asset_access_registry(&mut world);
        assert!(world.contains_resource::<AssetAccessRegistry>());
    }

    #[test]
    fn resource_gate_allows_readers_and_excludes_writers() {
        let registry = AssetAccessRegistry::default();
        let state = registry.state_for(TypeId::of::<MeshAsset>(), "Mesh");

        let first = state.try_read().expect("first reader");
        let second = state.try_read().expect("second reader");
        assert!(matches!(
            state.try_write(),
            Err(crate::StorageError::AssetAccessConflict)
        ));
        drop(first);
        drop(second);

        let writer = state.try_write().expect("writer after readers drop");
        assert!(matches!(
            state.try_read(),
            Err(crate::StorageError::AssetAccessConflict)
        ));
        assert!(matches!(
            state.try_write(),
            Err(crate::StorageError::AssetAccessConflict)
        ));
        drop(writer);
        assert!(state.try_read().is_ok());
    }

    #[test]
    fn write_guard_identity_is_world_and_asset_resource_specific() {
        let first_registry = AssetAccessRegistry::default();
        let second_registry = AssetAccessRegistry::default();
        let first = first_registry.state_for(TypeId::of::<MeshAsset>(), "Mesh");
        let same = first_registry.state_for(TypeId::of::<MeshAsset>(), "Mesh");
        let other_type = first_registry.state_for(TypeId::of::<ImageAsset>(), "Image");
        let other_world = second_registry.state_for(TypeId::of::<MeshAsset>(), "Mesh");
        let guard = first.try_write().expect("write guard");

        assert!(guard.authorizes(&same));
        assert!(!guard.authorizes(&other_type));
        assert!(!guard.authorizes(&other_world));
    }

    #[test]
    fn supersession_epoch_saturates_instead_of_wrapping() {
        let registry = AssetAccessRegistry::default();
        let state = registry.state_for(TypeId::of::<MeshAsset>(), "Mesh");
        state.set_epoch(u64::MAX - 1);

        assert_eq!(state.advance_epoch(), u64::MAX);
        assert_eq!(state.advance_epoch(), u64::MAX);
        assert_eq!(state.epoch(), u64::MAX);
        assert!(!AssetResourceState::epoch_is_cacheable(state.epoch()));
    }
}
