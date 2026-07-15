//! Backend-neutral identity and ownership transitions for Bevy [`App`] values.
//!
//! Interpreter adapters choose where to keep an [`AppStoreCore`] and translate
//! [`AppStoreError`] into their own exception types. This module deliberately
//! contains no Python or interpreter state.

use std::{
    collections::HashMap,
    fmt,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use bevy::prelude::App;

#[cfg(not(target_has_atomic = "64"))]
compile_error!("AppStoreCore requires target support for 64-bit atomics");

static APP_ID_ALLOCATOR: AppIdAllocator = AppIdAllocator::new(1);

pub type DrainedApps = Vec<(AppId, App)>;
pub type BorrowedApps = Vec<(AppId, AppOperation)>;

/// Process-wide identity for one logical PyBevy App wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AppId(NonZeroU64);

impl AppId {
    /// Return the stable numeric representation used in diagnostics.
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A fresh, single-use App ID capability.
///
/// The token is intentionally neither [`Clone`] nor [`Copy`]. Installing an
/// App consumes it, so safe code cannot reuse an ID in a second store.
///
/// ```compile_fail
/// use pybevy_storage::allocate_id;
///
/// let token = allocate_id().unwrap();
/// let duplicate = token.clone();
/// ```
pub struct AllocatedAppId {
    id: AppId,
}

impl AllocatedAppId {
    /// Inspect the ID without consuming its single-use capability.
    pub fn id(&self) -> AppId {
        self.id
    }
}

/// Allocate an ID from the process-wide, non-wrapping sequence.
pub fn allocate_id() -> Result<AllocatedAppId, AppStoreError> {
    APP_ID_ALLOCATOR.allocate().map(|id| AllocatedAppId { id })
}

/// Permanently consume an ID without installing an App.
///
/// Collection-only wrappers use this path. Because the fresh token is moved,
/// the returned ID can never subsequently be passed to [`AppStoreCore::insert_with_id`].
pub fn consume_unstored_id(allocated: AllocatedAppId) -> AppId {
    allocated.id
}

/// An operation that temporarily extracts an App from its store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppOperation {
    Update,
    Run,
    RunSchedule,
    WorldCallback,
    PluginCallback,
    BridgeBuild,
    Finish,
    Cleanup,
}

impl fmt::Display for AppOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Update => "update",
            Self::Run => "run",
            Self::RunSchedule => "run schedule",
            Self::WorldCallback => "world callback",
            Self::PluginCallback => "plugin callback",
            Self::BridgeBuild => "bridge build",
            Self::Finish => "finish",
            Self::Cleanup => "cleanup",
        })
    }
}

/// Public projection of an App slot's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLifecycle {
    Active,
    Borrowed(AppOperation),
    Consumed,
    Removed,
}

enum AppSlot {
    Active(Box<App>),
    Borrowed(AppOperation),
    Consumed,
    Removed,
}

/// Interpreter-neutral App storage failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStoreError {
    Missing(AppId),
    Borrowed(AppOperation),
    Consumed,
    Removed,
    IdExhausted,
    DuplicateId(AppId),
    NotBorrowed(AppLifecycle),
}

impl fmt::Display for AppStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(id) => write!(f, "App {id} does not exist"),
            Self::Borrowed(operation) => {
                write!(f, "App is already executing {operation}")
            }
            Self::Consumed => f.write_str("App has already been consumed"),
            Self::Removed => f.write_str("App has already been removed"),
            Self::IdExhausted => f.write_str("App ID space is exhausted"),
            Self::DuplicateId(id) => write!(f, "App ID {id} was already used"),
            Self::NotBorrowed(state) => {
                write!(f, "cannot restore App from {state:?} state")
            }
        }
    }
}

impl std::error::Error for AppStoreError {}

/// A failed restoration that retains ownership of the extracted App.
///
/// Adapters should treat this as a fatal invariant violation. Keeping the App
/// in the error prevents an accidental drop or second owner while reporting it.
pub struct AppRestoreError {
    error: AppStoreError,
    app: Box<App>,
}

impl AppRestoreError {
    pub fn error(&self) -> AppStoreError {
        self.error
    }

    pub fn into_parts(self) -> (AppStoreError, App) {
        (self.error, *self.app)
    }
}

impl fmt::Debug for AppRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppRestoreError")
            .field("error", &self.error)
            .field("app", &"App { .. }")
            .finish()
    }
}

impl fmt::Display for AppRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for AppRestoreError {}

/// Apps moved out during shutdown plus slots that could not be drained.
pub struct DrainOutcome {
    drained: DrainedApps,
    borrowed: BorrowedApps,
}

impl DrainOutcome {
    pub fn into_parts(self) -> (DrainedApps, BorrowedApps) {
        (self.drained, self.borrowed)
    }

    pub fn drained(&self) -> &[(AppId, App)] {
        &self.drained
    }

    pub fn borrowed(&self) -> &[(AppId, AppOperation)] {
        &self.borrowed
    }
}

/// Plain backend-owned container for App identities and ownership states.
#[derive(Default)]
pub struct AppStoreCore {
    slots: HashMap<AppId, AppSlot>,
}

impl AppStoreCore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, app: App) -> Result<AppId, AppStoreError> {
        self.insert_with_id(allocate_id()?, app)
    }

    /// Install an App using a fresh, single-use token.
    ///
    /// ```compile_fail
    /// use bevy::prelude::App;
    /// use pybevy_storage::{allocate_id, AppStoreCore};
    ///
    /// let mut store = AppStoreCore::new();
    /// let id = allocate_id().unwrap().id();
    /// store.insert_with_id(id, App::new()).unwrap();
    /// ```
    pub fn insert_with_id(
        &mut self,
        allocated: AllocatedAppId,
        app: App,
    ) -> Result<AppId, AppStoreError> {
        let id = allocated.id;
        if self.slots.contains_key(&id) {
            return Err(AppStoreError::DuplicateId(id));
        }
        self.slots.insert(id, AppSlot::Active(Box::new(app)));
        Ok(id)
    }

    /// Whether this store has ever held the ID, including tombstones.
    pub fn contains(&self, id: AppId) -> bool {
        self.slots.contains_key(&id)
    }

    /// Number of Apps currently owned by active slots.
    pub fn active_count(&self) -> usize {
        self.slots
            .values()
            .filter(|slot| matches!(slot, AppSlot::Active(_)))
            .count()
    }

    pub fn state(&self, id: AppId) -> Result<AppLifecycle, AppStoreError> {
        self.slots
            .get(&id)
            .map(AppSlot::lifecycle)
            .ok_or(AppStoreError::Missing(id))
    }

    /// Access an App only for code proven not to enter an interpreter or recurse.
    pub fn with_app_leaf<R>(
        &mut self,
        id: AppId,
        f: impl FnOnce(&mut App) -> R,
    ) -> Result<R, AppStoreError> {
        match self.slots.get_mut(&id) {
            Some(AppSlot::Active(app)) => Ok(f(app)),
            Some(slot) => Err(slot.access_error()),
            None => Err(AppStoreError::Missing(id)),
        }
    }

    pub fn begin_operation(
        &mut self,
        id: AppId,
        operation: AppOperation,
    ) -> Result<App, AppStoreError> {
        let slot = self.slots.get_mut(&id).ok_or(AppStoreError::Missing(id))?;
        match std::mem::replace(slot, AppSlot::Borrowed(operation)) {
            AppSlot::Active(app) => Ok(*app),
            previous => {
                let error = previous.access_error();
                *slot = previous;
                Err(error)
            }
        }
    }

    pub fn restore_operation(&mut self, id: AppId, app: App) -> Result<(), AppRestoreError> {
        let Some(slot) = self.slots.get_mut(&id) else {
            return Err(AppRestoreError {
                error: AppStoreError::Missing(id),
                app: Box::new(app),
            });
        };
        if !matches!(slot, AppSlot::Borrowed(_)) {
            return Err(AppRestoreError {
                error: AppStoreError::NotBorrowed(slot.lifecycle()),
                app: Box::new(app),
            });
        }
        *slot = AppSlot::Active(Box::new(app));
        Ok(())
    }

    /// Complete an extracted consuming operation without restoring its App.
    ///
    /// The adapter still owns the sole App value and must destroy it outside
    /// the store borrow. This transition leaves the slot's consumed tombstone
    /// visible to stale wrappers after `App::run` returns.
    pub fn finish_operation_consumed(&mut self, id: AppId) -> Result<(), AppStoreError> {
        let slot = self.slots.get_mut(&id).ok_or(AppStoreError::Missing(id))?;
        match std::mem::replace(slot, AppSlot::Consumed) {
            AppSlot::Borrowed(_) => Ok(()),
            previous => {
                let lifecycle = previous.lifecycle();
                *slot = previous;
                Err(AppStoreError::NotBorrowed(lifecycle))
            }
        }
    }

    pub fn take_for_run(&mut self, id: AppId) -> Result<App, AppStoreError> {
        let slot = self.slots.get_mut(&id).ok_or(AppStoreError::Missing(id))?;
        match std::mem::replace(slot, AppSlot::Consumed) {
            AppSlot::Active(app) => Ok(*app),
            previous => {
                let error = previous.access_error();
                *slot = previous;
                Err(error)
            }
        }
    }

    pub fn remove(&mut self, id: AppId) -> Result<Option<App>, AppStoreError> {
        let slot = self.slots.get_mut(&id).ok_or(AppStoreError::Missing(id))?;
        match std::mem::replace(slot, AppSlot::Removed) {
            AppSlot::Active(app) => Ok(Some(*app)),
            AppSlot::Consumed => {
                *slot = AppSlot::Consumed;
                Ok(None)
            }
            AppSlot::Removed => Ok(None),
            AppSlot::Borrowed(operation) => {
                *slot = AppSlot::Borrowed(operation);
                Err(AppStoreError::Borrowed(operation))
            }
        }
    }

    pub fn drain_active(&mut self) -> DrainOutcome {
        let mut drained = Vec::new();
        let mut borrowed = Vec::new();
        for (&id, slot) in &mut self.slots {
            match slot {
                AppSlot::Active(_) => {
                    if let AppSlot::Active(app) = std::mem::replace(slot, AppSlot::Removed) {
                        drained.push((id, *app));
                    }
                }
                AppSlot::Borrowed(operation) => borrowed.push((id, *operation)),
                AppSlot::Consumed | AppSlot::Removed => {}
            }
        }
        drained.sort_unstable_by_key(|(id, _)| *id);
        borrowed.sort_unstable_by_key(|(id, _)| *id);
        DrainOutcome { drained, borrowed }
    }
}

impl AppSlot {
    fn lifecycle(&self) -> AppLifecycle {
        match self {
            Self::Active(_) => AppLifecycle::Active,
            Self::Borrowed(operation) => AppLifecycle::Borrowed(*operation),
            Self::Consumed => AppLifecycle::Consumed,
            Self::Removed => AppLifecycle::Removed,
        }
    }

    fn access_error(&self) -> AppStoreError {
        match self {
            Self::Active(_) => unreachable!("active App slots are accessible"),
            Self::Borrowed(operation) => AppStoreError::Borrowed(*operation),
            Self::Consumed => AppStoreError::Consumed,
            Self::Removed => AppStoreError::Removed,
        }
    }
}

struct AppIdAllocator {
    next: AtomicU64,
}

impl AppIdAllocator {
    const fn new(next: u64) -> Self {
        Self {
            next: AtomicU64::new(next),
        }
    }

    fn allocate(&self) -> Result<AppId, AppStoreError> {
        let current = self
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| match next {
                0 => None,
                u64::MAX => Some(0),
                _ => Some(next + 1),
            })
            .map_err(|_| AppStoreError::IdExhausted)?;
        let id = NonZeroU64::new(current).ok_or(AppStoreError::IdExhausted)?;
        Ok(AppId(id))
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use bevy::prelude::{App, Resource};

    use super::*;

    #[derive(Resource)]
    struct Marker(u32);

    fn app_with(value: u32) -> App {
        let mut app = App::new();
        app.insert_resource(Marker(value));
        app
    }

    fn marker(store: &mut AppStoreCore, id: AppId) -> u32 {
        store
            .with_app_leaf(id, |app| app.world().resource::<Marker>().0)
            .unwrap()
    }

    #[test]
    fn allocator_is_monotonic_distinct_and_does_not_wrap() {
        let allocator = AppIdAllocator::new(41);
        assert_eq!(allocator.allocate().unwrap().get(), 41);
        assert_eq!(allocator.allocate().unwrap().get(), 42);

        let allocator = AppIdAllocator::new(u64::MAX);
        assert_eq!(allocator.allocate().unwrap().get(), u64::MAX);
        assert_eq!(allocator.allocate(), Err(AppStoreError::IdExhausted));
        assert_eq!(allocator.allocate(), Err(AppStoreError::IdExhausted));
    }

    #[test]
    fn exact_ids_select_independent_apps() {
        let mut store = AppStoreCore::new();
        let first = store.insert(app_with(1)).unwrap();
        let second = store.insert(app_with(2)).unwrap();
        assert_eq!(store.active_count(), 2);

        store
            .with_app_leaf(first, |app| app.world_mut().resource_mut::<Marker>().0 = 10)
            .unwrap();

        assert_eq!(marker(&mut store, first), 10);
        assert_eq!(marker(&mut store, second), 2);
        let absent = consume_unstored_id(allocate_id().unwrap());
        assert_eq!(
            store.with_app_leaf(absent, |_| ()),
            Err(AppStoreError::Missing(absent))
        );
    }

    #[test]
    fn consumed_and_removed_ids_never_fall_back() {
        let mut store = AppStoreCore::new();
        let consumed = store.insert(app_with(1)).unwrap();
        let active = store.insert(app_with(2)).unwrap();
        drop(store.take_for_run(consumed).unwrap());

        assert_eq!(store.state(consumed), Ok(AppLifecycle::Consumed));
        assert_eq!(
            store.with_app_leaf(consumed, |_| ()),
            Err(AppStoreError::Consumed)
        );
        assert_eq!(marker(&mut store, active), 2);
        assert!(store.remove(consumed).unwrap().is_none());
        assert!(store.remove(consumed).unwrap().is_none());

        drop(store.remove(active).unwrap());
        assert_eq!(store.state(active), Ok(AppLifecycle::Removed));
        assert!(store.remove(active).unwrap().is_none());
        assert_eq!(
            store.with_app_leaf(active, |_| ()),
            Err(AppStoreError::Removed)
        );
    }

    #[test]
    fn borrowed_operation_rejects_recursive_admission_and_removal() {
        let mut store = AppStoreCore::new();
        let id = store.insert(app_with(1)).unwrap();
        let app = store.begin_operation(id, AppOperation::Update).unwrap();

        assert_eq!(
            store.state(id),
            Ok(AppLifecycle::Borrowed(AppOperation::Update))
        );
        assert!(matches!(
            store.begin_operation(id, AppOperation::RunSchedule),
            Err(AppStoreError::Borrowed(AppOperation::Update))
        ));
        assert!(matches!(
            store.remove(id),
            Err(AppStoreError::Borrowed(AppOperation::Update))
        ));

        store.restore_operation(id, app).unwrap();
        assert_eq!(store.state(id), Ok(AppLifecycle::Active));
    }

    #[test]
    fn consuming_operation_transitions_borrowed_slot_to_consumed() {
        let mut store = AppStoreCore::new();
        let id = store.insert(app_with(1)).unwrap();
        let app = store.begin_operation(id, AppOperation::Run).unwrap();

        store.finish_operation_consumed(id).unwrap();
        drop(app);

        assert_eq!(store.state(id), Ok(AppLifecycle::Consumed));
        assert_eq!(
            store.finish_operation_consumed(id),
            Err(AppStoreError::NotBorrowed(AppLifecycle::Consumed))
        );
    }

    #[test]
    fn adapter_style_guard_restores_after_unwind() {
        struct Guard<'a> {
            store: &'a mut AppStoreCore,
            id: AppId,
            app: Option<App>,
        }

        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                let app = self.app.take().unwrap();
                self.store.restore_operation(self.id, app).unwrap();
            }
        }

        let mut store = AppStoreCore::new();
        let id = store.insert(app_with(1)).unwrap();
        let app = store.begin_operation(id, AppOperation::Update).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = Guard {
                store: &mut store,
                id,
                app: Some(app),
            };
            panic!("callback panic");
        }));

        assert!(result.is_err());
        assert_eq!(store.state(id), Ok(AppLifecycle::Active));
        assert_eq!(marker(&mut store, id), 1);
    }

    #[test]
    fn failed_restore_retains_app_ownership() {
        let mut store = AppStoreCore::new();
        let id = store.insert(app_with(1)).unwrap();
        let app = app_with(2);
        let error = store.restore_operation(id, app).unwrap_err();
        assert_eq!(
            error.error(),
            AppStoreError::NotBorrowed(AppLifecycle::Active)
        );
        let (_, mut retained) = error.into_parts();
        assert_eq!(retained.world_mut().resource::<Marker>().0, 2);
        assert_eq!(marker(&mut store, id), 1);
    }

    #[test]
    fn drain_returns_apps_and_reports_borrowed_slots() {
        let mut store = AppStoreCore::new();
        let first = store.insert(app_with(1)).unwrap();
        let second = store.insert(app_with(2)).unwrap();
        let borrowed_app = store
            .begin_operation(second, AppOperation::Cleanup)
            .unwrap();
        assert_eq!(store.active_count(), 1);

        let outcome = store.drain_active();
        assert_eq!(outcome.drained().len(), 1);
        assert_eq!(outcome.drained()[0].0, first);
        assert_eq!(outcome.borrowed(), &[(second, AppOperation::Cleanup)]);
        assert_eq!(store.state(first), Ok(AppLifecycle::Removed));
        assert_eq!(
            store.state(second),
            Ok(AppLifecycle::Borrowed(AppOperation::Cleanup))
        );

        store.restore_operation(second, borrowed_app).unwrap();
        assert_eq!(store.active_count(), 1);
        let (drained, borrowed) = store.drain_active().into_parts();
        assert_eq!(drained.len(), 1);
        assert!(borrowed.is_empty());
        assert_eq!(store.active_count(), 0);
        assert_eq!(store.state(second), Ok(AppLifecycle::Removed));
    }

    #[test]
    fn standalone_and_cross_store_ids_have_no_global_store_coupling() {
        let unused = consume_unstored_id(allocate_id().unwrap());
        let token = allocate_id().unwrap();
        let installed = token.id();
        let mut first_store = AppStoreCore::new();
        let second_store = AppStoreCore::new();
        first_store.insert_with_id(token, app_with(1)).unwrap();

        assert!(first_store.contains(installed));
        assert!(!first_store.contains(unused));
        assert!(!second_store.contains(installed));
        assert_eq!(
            second_store.state(installed),
            Err(AppStoreError::Missing(installed))
        );
    }

    #[test]
    fn runtime_defense_rejects_current_and_historical_slot_collisions() {
        fn forged_test_token(id: AppId) -> AllocatedAppId {
            // This helper is possible only inside the defining module. Public
            // safe code cannot construct or duplicate an allocation token.
            AllocatedAppId { id }
        }

        let mut store = AppStoreCore::new();
        let token = allocate_id().unwrap();
        let id = token.id();
        store.insert_with_id(token, app_with(1)).unwrap();

        assert_eq!(
            store.insert_with_id(forged_test_token(id), app_with(2)),
            Err(AppStoreError::DuplicateId(id))
        );
        drop(store.remove(id).unwrap());
        assert_eq!(
            store.insert_with_id(forged_test_token(id), app_with(3)),
            Err(AppStoreError::DuplicateId(id))
        );
    }
}
