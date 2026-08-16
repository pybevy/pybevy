//! Runtime validity checking for system parameters
//!
//! This module provides a robust pattern for ensuring system parameters
//! (like World, Commands, Assets) are only accessed during system execution.
//!
//! The pattern uses Arc<AtomicU8> flags that track access mode (Read, Write, or Invalid)
//! and are automatically invalidated when the system completes (via RAII).
//!
//! # Thread affinity
//!
//! Passing the atomic mode check is not a lock on the pointed-to data: between
//! a `check()` returning valid and the caller's raw-pointer dereference, another
//! thread could invalidate the flag and the executor could free/re-alias that
//! data. So each flag is also pinned to the thread that activated it. A wrapper
//! shared to any other thread (a Python-spawned thread, or a second Python
//! system running concurrently on a worker thread that pulled the wrapper from a
//! global) fails its validity check before any dereference. Legitimate use is
//! always same-thread: a system's parameters are created, used, and invalidated
//! on the one thread that runs the system.

use std::{
    fmt,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
};

use crate::{component_change::ComponentWriteContext, storage_error::StorageError};

/// Source of process-unique thread tokens. Starts at 1 so the value 0 is a
/// reserved "unset" sentinel that can never collide with a live thread.
static NEXT_THREAD_TOKEN: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// A stable, process-unique id for the current OS thread, assigned lazily
    /// on first use. Cheaper and more portable than `ThreadId` (which is opaque
    /// and not storable in an atomic).
    static THREAD_TOKEN: u64 = NEXT_THREAD_TOKEN.fetch_add(1, Ordering::Relaxed);
}

#[inline]
fn current_thread_token() -> u64 {
    THREAD_TOKEN.with(|t| *t)
}

/// Access mode for system parameters and query components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessMode {
    /// Invalid - parameter cannot be used (system not executing)
    Invalid = 0,
    /// Read-only access - can read but not write
    Read = 1,
    /// Mutable access - can both read and write
    Write = 2,
}

impl From<u8> for AccessMode {
    fn from(value: u8) -> Self {
        match value {
            1 => AccessMode::Read,
            2 => AccessMode::Write,
            _ => AccessMode::Invalid,
        }
    }
}

/// Shared inner state of a [`ValidityFlag`].
struct ValidityInner {
    /// Current [`AccessMode`] (Invalid / Read / Write).
    mode: AtomicU8,
    /// Token of the thread that last activated this flag (0 while Invalid/unset).
    owner: AtomicU64,
    has_invalidation_observers: AtomicBool,
    invalidation_observers: Mutex<Vec<Weak<dyn InvalidationObserver>>>,
}

impl fmt::Debug for ValidityInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidityInner")
            .field("mode", &self.mode.load(Ordering::Relaxed))
            .field("owner", &self.owner.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

/// A shared-core listener that releases run-scoped bookkeeping immediately
/// when its validity window closes.
pub(crate) trait InvalidationObserver: Send + Sync {
    fn invalidated(&self);
}

impl ValidityInner {
    fn new(mode: AccessMode) -> Self {
        let owner = if matches!(mode, AccessMode::Invalid) {
            0
        } else {
            current_thread_token()
        };
        Self {
            mode: AtomicU8::new(mode as u8),
            owner: AtomicU64::new(owner),
            has_invalidation_observers: AtomicBool::new(false),
            invalidation_observers: Mutex::new(Vec::new()),
        }
    }
}

/// A validity flag that can be shared across multiple system parameters
/// and checked to ensure they're only used during system execution.
///
/// Also tracks whether the parameter has read-only or mutable access,
/// enabling runtime enforcement of Bevy's read/write semantics, and the thread
/// that activated it, so a wrapper shared to another thread is rejected before
/// any dereference (see module docs).
#[derive(Debug, Clone)]
pub struct ValidityFlag(Arc<ValidityInner>);

/// A wrapper around ValidityFlag that enforces a specific access mode
///
/// This shares the same validity state (via Arc) as the master ValidityFlag,
/// so it gets invalidated when the system exits (RAII), but enforces
/// a specific access mode (Read or Write) for this particular component.
#[derive(Debug, Clone)]
pub struct ValidityFlagWithMode {
    pub flag: ValidityFlag,
    access_mode: AccessMode,
    component_write: Option<ComponentWriteContext>,
}

impl ValidityFlagWithMode {
    /// Check if reading is allowed
    pub fn check_read(&self) -> Result<(), StorageError> {
        // Still valid (not invalidated by system exit) and on the owning thread?
        if matches!(self.flag.get_mode(), AccessMode::Invalid) {
            return Err(StorageError::InvalidAccess);
        }
        self.flag.check_thread()?;
        // Valid on this thread; now check our access mode allows reading.
        match self.access_mode {
            AccessMode::Read | AccessMode::Write => Ok(()),
            AccessMode::Invalid => {
                unreachable!("ValidityFlagWithMode should never have Invalid mode")
            }
        }
    }

    /// Check if writing is allowed
    pub fn check_write(&self) -> Result<(), StorageError> {
        // Still valid (not invalidated by system exit) and on the owning thread?
        if matches!(self.flag.get_mode(), AccessMode::Invalid) {
            return Err(StorageError::InvalidAccess);
        }
        self.flag.check_thread()?;
        // Valid on this thread; now check our access mode allows writing.
        match self.access_mode {
            AccessMode::Write => Ok(()),
            AccessMode::Read => Err(StorageError::ReadOnly),
            AccessMode::Invalid => {
                unreachable!("ValidityFlagWithMode should never have Invalid mode")
            }
        }
    }

    /// Legacy check method for backward compatibility (checks read access)
    pub fn check(&self) -> Result<(), StorageError> {
        self.check_read()
    }

    /// Get the access mode for this validity flag
    pub fn access_mode(&self) -> AccessMode {
        self.access_mode
    }

    /// Attach the native component identity used for lazy write-time change tracking.
    pub fn with_component_write_context(mut self, context: ComponentWriteContext) -> Self {
        self.component_write = Some(context);
        self
    }

    /// Return the native component write context, when this wrapper came from a query.
    pub fn component_write_context(&self) -> Option<ComponentWriteContext> {
        self.component_write
    }
}

impl ValidityFlag {
    /// Create a new validity flag, initially set to Invalid
    pub fn new() -> Self {
        Self(Arc::new(ValidityInner::new(AccessMode::Invalid)))
    }

    /// Create a new validity flag for read-only access, owned by the current thread
    pub fn new_read() -> Self {
        Self(Arc::new(ValidityInner::new(AccessMode::Read)))
    }

    /// Create a new validity flag for mutable (read+write) access, owned by the current thread
    pub fn new_write() -> Self {
        Self(Arc::new(ValidityInner::new(AccessMode::Write)))
    }

    /// Create a wrapper that shares the same validity state but enforces a specific access mode
    ///
    /// This is used for query parameters where the master validity is managed by ValidityGuard,
    /// but each parameter needs its own read/write restrictions.
    pub fn with_access_mode(&self, access_mode: AccessMode) -> ValidityFlagWithMode {
        ValidityFlagWithMode {
            flag: self.clone(),
            access_mode,
            component_write: None,
        }
    }

    /// Get the current access mode
    pub fn get_mode(&self) -> AccessMode {
        self.0.mode.load(Ordering::Acquire).into()
    }

    /// Notify `observer` when this validity window becomes invalid.
    ///
    /// Registration after invalidation invokes the observer immediately. An
    /// observer must therefore make its cleanup idempotent.
    pub(crate) fn observe_invalidation(&self, observer: &Arc<dyn InvalidationObserver>) {
        if matches!(self.get_mode(), AccessMode::Invalid) {
            observer.invalidated();
            return;
        }

        {
            let mut observers = self
                .0
                .invalidation_observers
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            observers.push(Arc::downgrade(observer));
            self.0
                .has_invalidation_observers
                .store(true, Ordering::Release);
        }

        if matches!(self.get_mode(), AccessMode::Invalid) {
            observer.invalidated();
        }
    }

    /// Require that the caller is on the thread that activated this flag.
    ///
    /// Called only after confirming a valid mode. Loading `owner` after the
    /// Acquire load of `mode` (in `get_mode`) observes the owner stored before
    /// the Release store of a valid mode in `set_mode`.
    fn check_thread(&self) -> Result<(), StorageError> {
        if self.0.owner.load(Ordering::Relaxed) == current_thread_token() {
            Ok(())
        } else {
            Err(StorageError::CrossThreadAccess)
        }
    }

    /// Check if the flag allows read access
    ///
    /// Returns Ok(()) if valid for reading (Read or Write mode) and accessed
    /// from the owning thread; Err otherwise.
    pub fn check_read(&self) -> Result<(), StorageError> {
        match self.get_mode() {
            AccessMode::Read | AccessMode::Write => self.check_thread(),
            AccessMode::Invalid => Err(StorageError::InvalidAccess),
        }
    }

    /// Check if the flag allows write access
    ///
    /// Returns Ok(()) if valid for writing (Write mode only) and accessed from
    /// the owning thread; Err otherwise.
    pub fn check_write(&self) -> Result<(), StorageError> {
        match self.get_mode() {
            AccessMode::Write => self.check_thread(),
            AccessMode::Read => Err(StorageError::ReadOnly),
            AccessMode::Invalid => Err(StorageError::InvalidAccess),
        }
    }

    /// Legacy check method for backward compatibility (checks read access)
    ///
    /// Returns Ok(()) if valid for reading, Err if Invalid.
    pub fn check(&self) -> Result<(), StorageError> {
        self.check_read()
    }

    /// Set the validity flag to a specific access mode.
    ///
    /// Activating (any valid mode) re-pins the flag to the current thread:
    /// `owner` is stored before the Release store of `mode`, so a reader that
    /// observes the valid mode via `get_mode`'s Acquire load also sees this
    /// owner. Invalidation leaves `owner` untouched; the mode gate runs first.
    fn set_mode(&self, mode: AccessMode) {
        if !matches!(mode, AccessMode::Invalid) {
            self.0
                .owner
                .store(current_thread_token(), Ordering::Relaxed);
        }
        self.0.mode.store(mode as u8, Ordering::Release);
    }

    /// Set the validity flag to Write mode
    fn set_valid(&self) {
        self.set_mode(AccessMode::Write);
    }

    /// Set the validity flag to Invalid
    pub fn set_invalid(&self) {
        self.set_mode(AccessMode::Invalid);
        if !self.0.has_invalidation_observers.load(Ordering::Acquire) {
            return;
        }
        let observers = {
            let mut registered = self
                .0
                .invalidation_observers
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let observers = registered
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>();
            registered.clear();
            self.0
                .has_invalidation_observers
                .store(false, Ordering::Release);
            observers
        };
        for observer in observers {
            observer.invalidated();
        }
    }
}

impl Default for ValidityFlag {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard that sets a validity flag to true on creation
/// and automatically sets it to false when dropped.
///
/// This ensures system parameters are invalidated even if the
/// Python code panics or errors.
pub struct ValidityGuard {
    flag: ValidityFlag,
}

impl ValidityGuard {
    /// Create a new guard for the given validity flag.
    ///
    /// The flag is immediately set to valid (true).
    pub fn new(flag: ValidityFlag) -> Self {
        flag.set_valid();
        Self { flag }
    }
}

impl Drop for ValidityGuard {
    fn drop(&mut self) {
        // This runs even if the Python code panics!
        self.flag.set_invalid();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validity_flag_starts_invalid() {
        let flag = ValidityFlag::new();
        assert!(flag.check().is_err());
        assert!(flag.check_read().is_err());
        assert!(flag.check_write().is_err());
    }

    #[test]
    fn test_validity_guard_enables_flag() {
        let flag = ValidityFlag::new();
        let _guard = ValidityGuard::new(flag.clone());
        assert!(flag.check().is_ok());
        assert!(flag.check_read().is_ok());
        assert!(flag.check_write().is_ok()); // ValidityGuard sets to Write mode
    }

    #[test]
    fn test_validity_guard_disables_on_drop() {
        let flag = ValidityFlag::new();
        {
            let _guard = ValidityGuard::new(flag.clone());
            assert!(flag.check().is_ok());
        } // guard dropped here
        assert!(flag.check().is_err());
    }

    #[test]
    fn test_validity_flag_can_be_cloned() {
        let flag1 = ValidityFlag::new();
        let flag2 = flag1.clone();

        let _guard = ValidityGuard::new(flag1.clone());

        // Both clones see the same state
        assert!(flag1.check().is_ok());
        assert!(flag2.check().is_ok());
    }

    #[test]
    fn test_read_only_flag() {
        let flag = ValidityFlag::new_read();

        // Read access should work
        assert!(flag.check_read().is_ok());

        // Write access should fail
        assert!(flag.check_write().is_err());
    }

    #[test]
    fn test_write_flag() {
        let flag = ValidityFlag::new_write();

        // Both read and write should work
        assert!(flag.check_read().is_ok());
        assert!(flag.check_write().is_ok());
    }

    #[test]
    fn test_access_mode_error_cases() {
        // Read-only flag should reject writes
        let read_flag = ValidityFlag::new_read();
        assert!(read_flag.check_read().is_ok());
        assert!(read_flag.check_write().is_err());

        // Invalid flag should reject both reads and writes
        let invalid_flag = ValidityFlag::new();
        assert!(invalid_flag.check_read().is_err());
        assert!(invalid_flag.check_write().is_err());

        // Write flag should accept both
        let write_flag = ValidityFlag::new_write();
        assert!(write_flag.check_read().is_ok());
        assert!(write_flag.check_write().is_ok());
    }

    #[test]
    fn test_validity_flag_with_mode_error_cases() {
        // ValidityFlagWithMode with Read mode should reject writes even when flag is valid
        let flag = ValidityFlag::new_write(); // valid flag
        let read_mode = flag.with_access_mode(AccessMode::Read);
        assert!(read_mode.check_read().is_ok());
        assert!(read_mode.check_write().is_err());

        // ValidityFlagWithMode with Write mode should allow both
        let write_mode = flag.with_access_mode(AccessMode::Write);
        assert!(write_mode.check_read().is_ok());
        assert!(write_mode.check_write().is_ok());

        // When underlying flag becomes Invalid, both modes should reject
        flag.set_invalid();
        assert!(read_mode.check_read().is_err());
        assert!(write_mode.check_read().is_err());
    }

    #[test]
    fn test_guard_invalidates_across_clones() {
        let flag = ValidityFlag::new();
        let clone1 = flag.clone();
        let clone2 = flag.clone();
        let with_mode = flag.with_access_mode(AccessMode::Write);

        {
            let _guard = ValidityGuard::new(flag.clone());
            assert!(clone1.check_read().is_ok());
            assert!(clone2.check_write().is_ok());
            assert!(with_mode.check_write().is_ok());
        }

        // Guard dropped: all clones and with_mode see Invalid
        assert!(clone1.check_read().is_err());
        assert!(clone2.check_write().is_err());
        assert!(with_mode.check_read().is_err());
    }

    #[test]
    fn check_rejects_use_from_another_thread() {
        // A valid flag used from the owning thread passes; the same Arc used from
        // a different thread is rejected before any dereference would happen.
        let flag = ValidityFlag::new_write();
        assert!(flag.check_read().is_ok());
        assert!(flag.check_write().is_ok());

        let other = flag.clone();
        let (read_err, write_err, legacy_err) = std::thread::spawn(move || {
            (
                other.check_read().is_err(),
                other.check_write().is_err(),
                other.check().is_err(),
            )
        })
        .join()
        .unwrap();
        assert!(read_err && write_err && legacy_err);

        // The owning thread still works after the cross-thread attempt.
        assert!(flag.check_write().is_ok());
    }

    #[test]
    fn cross_thread_access_uses_distinct_error() {
        let flag = ValidityFlag::new_write();
        let other = flag.clone();
        let err = std::thread::spawn(move || other.check_read().unwrap_err())
            .join()
            .unwrap();
        assert!(matches!(err, StorageError::CrossThreadAccess));
    }

    #[test]
    fn with_mode_rejects_use_from_another_thread() {
        let flag = ValidityFlag::new_write();
        let with_mode = flag.with_access_mode(AccessMode::Write);
        assert!(with_mode.check_write().is_ok());

        let other = with_mode.clone();
        let rejected = std::thread::spawn(move || {
            matches!(other.check_write(), Err(StorageError::CrossThreadAccess))
                && matches!(other.check_read(), Err(StorageError::CrossThreadAccess))
        })
        .join()
        .unwrap();
        assert!(rejected);
    }

    #[test]
    fn reactivation_re_pins_owner_to_current_thread() {
        // A flag activated on this thread, invalidated, then reactivated on
        // another thread, is owned by that other thread afterwards.
        let flag = ValidityFlag::new();
        {
            let _guard = ValidityGuard::new(flag.clone());
            assert!(flag.check_write().is_ok());
        }
        let moved = flag.clone();
        std::thread::spawn(move || {
            let _guard = ValidityGuard::new(moved.clone());
            // Reactivated here: valid on this worker thread.
            assert!(moved.check_write().is_ok());
        })
        .join()
        .unwrap();
        // Back on the original thread the flag is invalid (guard dropped).
        assert!(flag.check_read().is_err());
    }
}
