use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

use bevy::prelude::Resource;
use pybevy_reload::{HotReloadStateAccess, ReloadMode, ReloadRequestState, lock_or_recover};
use pyo3::prelude::*;

use super::util::is_verbose;

/// Shared state for hot reload functionality
/// Arc<Mutex<...>> allows thread-safe access from both Python watcher thread and Bevy systems
#[derive(Clone)]
pub struct HotReloadState {
    pub(crate) inner: Arc<Mutex<HotReloadStateInner>>,
    requests: ReloadRequestState,
}

pub(crate) struct HotReloadStateInner {
    /// Function to call to get fresh systems when reload is requested
    /// This should return a new create_app() function
    pub(crate) loader_func: Option<Py<PyAny>>,
    /// Default mode for file-triggered reloads (can be toggled with F6)
    pub(crate) default_reload_mode: ReloadMode,
    /// Current generation counter - systems check this to know if they should run
    /// This is atomic so it can be read without locking from Bevy systems
    pub(crate) generation: Arc<AtomicU32>,
}

impl HotReloadState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HotReloadStateInner {
                loader_func: None,
                default_reload_mode: ReloadMode::Partial,
                generation: Arc::new(AtomicU32::new(0)),
            })),
            requests: ReloadRequestState::default(),
        }
    }

    /// Create a new state with a specific generation (for hot reload temp apps)
    pub(crate) fn with_generation(generation: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HotReloadStateInner {
                loader_func: None,
                default_reload_mode: ReloadMode::Partial,
                generation: Arc::new(AtomicU32::new(generation)),
            })),
            requests: ReloadRequestState::default(),
        }
    }

    /// Get the current default reload mode
    pub fn get_default_mode(&self) -> ReloadMode {
        lock_or_recover(&self.inner).default_reload_mode
    }

    /// Set the default reload mode (for file-triggered reloads)
    pub fn set_default_mode(&self, mode: ReloadMode) {
        lock_or_recover(&self.inner).default_reload_mode = mode;
    }

    /// Get the current generation counter
    pub fn generation(&self) -> Arc<AtomicU32> {
        lock_or_recover(&self.inner).generation.clone()
    }

    /// Increment the generation counter (disables all old systems)
    pub fn increment_generation(&self) {
        let inner = lock_or_recover(&self.inner);
        inner.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Get the current generation value
    pub fn current_generation(&self) -> u32 {
        lock_or_recover(&self.inner)
            .generation
            .load(Ordering::SeqCst)
    }

    /// Set the generation counter to a specific value (for rollback on reload failure)
    pub fn set_generation(&self, generation: u32) {
        let inner = lock_or_recover(&self.inner);
        inner.generation.store(generation, Ordering::SeqCst);
    }

    /// Check if hot reload is enabled (loader function is set)
    pub fn is_enabled(&self) -> bool {
        let inner = lock_or_recover(&self.inner);
        inner.loader_func.is_some()
    }

    /// Check if the next reload will be in partial mode
    /// Used by CLI loader to determine whether to enable component caching
    pub fn is_partial_reload(&self) -> bool {
        let mode = self.requests.last_mode();
        let is_partial = mode == ReloadMode::Partial;
        if is_verbose() {
            eprintln!(
                "🔍 is_partial_reload() called: reload_mode = {:?}, returning {}",
                mode, is_partial
            );
        }
        is_partial
    }

    /// Set the loader function that will be called to recreate the app
    pub fn set_loader(&self, func: Py<PyAny>) {
        let old_loader = {
            let mut inner = lock_or_recover(&self.inner);
            inner.loader_func.replace(func)
        };
        drop(old_loader);
    }

    /// Request a reload (called from Python watcher thread)
    pub fn request_reload(&self, mode: ReloadMode) {
        self.requests.request(mode);
    }

    pub(crate) fn request_reload_if_idle(&self, mode: ReloadMode) -> bool {
        self.requests.request_if_idle(mode)
    }

    pub(crate) fn is_reload_pending(&self) -> bool {
        self.requests.is_pending()
    }

    /// Check if reload is pending and get the loader function and mode if available.
    /// A request taken before any loader is installed is intentionally consumed and dropped.
    pub fn take_pending_reload(&self, py: Python) -> Option<(Py<PyAny>, ReloadMode)> {
        let mode = self.requests.take()?;
        lock_or_recover(&self.inner)
            .loader_func
            .as_ref()
            .map(|function| (function.clone_ref(py), mode))
    }
}

impl HotReloadStateAccess for HotReloadState {
    fn current_generation(&self) -> u32 {
        HotReloadState::current_generation(self)
    }

    fn increment_generation(&self) {
        HotReloadState::increment_generation(self)
    }

    fn set_generation(&self, generation: u32) {
        HotReloadState::set_generation(self, generation)
    }
}

#[derive(Resource, Clone)]
pub struct HotReloadResource {
    pub state: HotReloadState,
    pub(crate) error_state: Arc<Mutex<Vec<PyErr>>>,
}

impl HotReloadResource {
    pub fn new(state: HotReloadState, error_state: Arc<Mutex<Vec<PyErr>>>) -> Self {
        Self { state, error_state }
    }
}
