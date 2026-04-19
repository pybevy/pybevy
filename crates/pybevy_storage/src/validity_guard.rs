//! Runtime validity checking for system parameters
//!
//! This module provides a robust pattern for ensuring system parameters
//! (like World, Commands, Assets) are only accessed during system execution.
//!
//! The pattern uses Arc<AtomicU8> flags that track access mode (Read, Write, or Invalid)
//! and are automatically invalidated when the system completes (via RAII).

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use crate::storage_error::StorageError;

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

/// A validity flag that can be shared across multiple system parameters
/// and checked to ensure they're only used during system execution.
///
/// Also tracks whether the parameter has read-only or mutable access,
/// enabling runtime enforcement of Bevy's read/write semantics.
#[derive(Debug, Clone)]
pub struct ValidityFlag(Arc<AtomicU8>);

/// A wrapper around ValidityFlag that enforces a specific access mode
///
/// This shares the same validity state (via Arc) as the master ValidityFlag,
/// so it gets invalidated when the system exits (RAII), but enforces
/// a specific access mode (Read or Write) for this particular component.
#[derive(Debug, Clone)]
pub struct ValidityFlagWithMode {
    pub flag: ValidityFlag,
    access_mode: AccessMode,
}

impl ValidityFlagWithMode {
    /// Check if reading is allowed
    pub fn check_read(&self) -> Result<(), StorageError> {
        // First check if we're still valid (not invalidated by system exit)
        if !matches!(self.flag.get_mode(), AccessMode::Invalid) {
            // We're valid, now check if our access mode allows reading
            match self.access_mode {
                AccessMode::Read | AccessMode::Write => Ok(()),
                AccessMode::Invalid => {
                    unreachable!("ValidityFlagWithMode should never have Invalid mode")
                }
            }
        } else {
            Err(StorageError::InvalidAccess)
        }
    }

    /// Check if writing is allowed
    pub fn check_write(&self) -> Result<(), StorageError> {
        // First check if we're still valid (not invalidated by system exit)
        if !matches!(self.flag.get_mode(), AccessMode::Invalid) {
            // We're valid, now check if our access mode allows writing
            match self.access_mode {
                AccessMode::Write => Ok(()),
                AccessMode::Read => Err(StorageError::ReadOnly),
                AccessMode::Invalid => {
                    unreachable!("ValidityFlagWithMode should never have Invalid mode")
                }
            }
        } else {
            Err(StorageError::InvalidAccess)
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
}

impl ValidityFlag {
    /// Create a new validity flag, initially set to Invalid
    pub fn new() -> Self {
        Self(Arc::new(AtomicU8::new(AccessMode::Invalid as u8)))
    }

    /// Create a new validity flag for read-only access
    pub fn new_read() -> Self {
        Self(Arc::new(AtomicU8::new(AccessMode::Read as u8)))
    }

    /// Create a new validity flag for mutable (read+write) access
    pub fn new_write() -> Self {
        Self(Arc::new(AtomicU8::new(AccessMode::Write as u8)))
    }

    /// Create a wrapper that shares the same validity state but enforces a specific access mode
    ///
    /// This is used for query parameters where the master validity is managed by ValidityGuard,
    /// but each parameter needs its own read/write restrictions.
    pub fn with_access_mode(&self, access_mode: AccessMode) -> ValidityFlagWithMode {
        ValidityFlagWithMode {
            flag: self.clone(),
            access_mode,
        }
    }

    /// Get the current access mode
    pub fn get_mode(&self) -> AccessMode {
        self.0.load(Ordering::Acquire).into()
    }

    /// Check if the flag allows read access
    ///
    /// Returns Ok(()) if valid for reading (Read or Write mode), Err if Invalid.
    pub fn check_read(&self) -> Result<(), StorageError> {
        match self.get_mode() {
            AccessMode::Read | AccessMode::Write => Ok(()),
            AccessMode::Invalid => Err(StorageError::InvalidAccess),
        }
    }

    /// Check if the flag allows write access
    ///
    /// Returns Ok(()) if valid for writing (Write mode only), Err otherwise.
    pub fn check_write(&self) -> Result<(), StorageError> {
        match self.get_mode() {
            AccessMode::Write => Ok(()),
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

    /// Set the validity flag to a specific access mode
    fn set_mode(&self, mode: AccessMode) {
        self.0.store(mode as u8, Ordering::Release);
    }

    /// Set the validity flag to Write mode
    fn set_valid(&self) {
        self.set_mode(AccessMode::Write);
    }

    /// Set the validity flag to Invalid
    pub fn set_invalid(&self) {
        self.set_mode(AccessMode::Invalid);
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

        // Guard dropped — all clones and with_mode see Invalid
        assert!(clone1.check_read().is_err());
        assert!(clone2.check_write().is_err());
        assert!(with_mode.check_read().is_err());
    }
}
