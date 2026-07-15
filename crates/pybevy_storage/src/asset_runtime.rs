//! Backend-neutral admission state for runtime `Assets[T]` parameters.
//!
//! Interpreter adapters retain their world cell, bridge lookup, Python object
//! conversion, and iterator protocol. This core owns only the safety-bearing
//! checks that must agree across adapters.

use std::fmt;

use crate::{AssetBorrowCounter, StorageError, ValidityFlagWithMode};

/// Neutral failures produced before an asset bridge or interpreter operation.
#[derive(Debug, Clone)]
pub enum AssetRuntimeError {
    /// The run-scoped validity or thread-affinity check failed.
    Access(StorageError),
    /// A write was attempted through `Res[Assets[T]]`.
    MutableAccessRequired,
    /// A handle belongs to another registered asset type.
    HandleTypeMismatch { actual: String, expected: String },
    /// Structural mutation would invalidate a live borrowed asset wrapper.
    BorrowedAssetsLive { asset_name: String },
}

impl AssetRuntimeError {
    /// Whether adapters should expose this failure as a Python `ValueError`.
    pub fn is_handle_type_mismatch(&self) -> bool {
        matches!(self, Self::HandleTypeMismatch { .. })
    }
}

impl fmt::Display for AssetRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => error.fmt(f),
            Self::MutableAccessRequired => f.write_str(
                "Mutable access required. Use ResMut[Assets[T]] instead of Res[Assets[T]] for mutations.",
            ),
            Self::HandleTypeMismatch { actual, expected } => write!(
                f,
                "Handle of type `{actual}` does not match expected type `{expected}`"
            ),
            Self::BorrowedAssetsLive { asset_name } => write!(
                f,
                "Cannot structurally mutate Assets[{asset_name}] while borrowed asset wrappers are live"
            ),
        }
    }
}

impl std::error::Error for AssetRuntimeError {}

/// Interpreter-neutral admission state for one runtime `Assets[T]` wrapper.
#[derive(Debug, Clone)]
pub struct AssetRuntimeCore<K> {
    type_key: K,
    asset_name: String,
    validity: ValidityFlagWithMode,
    borrow_counter: AssetBorrowCounter,
}

impl<K: Eq> AssetRuntimeCore<K> {
    pub fn new(
        type_key: K,
        asset_name: impl Into<String>,
        validity: ValidityFlagWithMode,
        borrow_counter: AssetBorrowCounter,
    ) -> Self {
        Self {
            type_key,
            asset_name: asset_name.into(),
            validity,
            borrow_counter,
        }
    }

    pub fn type_key(&self) -> &K {
        &self.type_key
    }

    pub fn asset_name(&self) -> &str {
        &self.asset_name
    }

    pub fn validity(&self) -> &ValidityFlagWithMode {
        &self.validity
    }

    pub fn borrow_counter(&self) -> &AssetBorrowCounter {
        &self.borrow_counter
    }

    pub fn check_read(&self) -> Result<(), AssetRuntimeError> {
        self.validity
            .check_read()
            .map_err(AssetRuntimeError::Access)
    }

    pub fn check_write(&self) -> Result<(), AssetRuntimeError> {
        self.validity.check_write().map_err(|error| match error {
            StorageError::ReadOnly => AssetRuntimeError::MutableAccessRequired,
            other => AssetRuntimeError::Access(other),
        })
    }

    pub fn check_handle_type(
        &self,
        actual_key: &K,
        actual_name: impl Into<String>,
    ) -> Result<(), AssetRuntimeError> {
        if actual_key == &self.type_key {
            return Ok(());
        }
        Err(AssetRuntimeError::HandleTypeMismatch {
            actual: actual_name.into(),
            expected: self.asset_name.clone(),
        })
    }

    pub fn check_no_live_asset_borrows(&self) -> Result<(), AssetRuntimeError> {
        self.check_read()?;
        if self.borrow_counter.has_active() {
            return Err(AssetRuntimeError::BorrowedAssetsLive {
                asset_name: self.asset_name.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccessMode, ValidityFlag};

    fn core(mode: AccessMode) -> AssetRuntimeCore<u64> {
        let validity = match mode {
            AccessMode::Read => ValidityFlag::new_read(),
            AccessMode::Write => ValidityFlag::new_write(),
            AccessMode::Invalid => ValidityFlag::new(),
        };
        AssetRuntimeCore::new(
            7,
            "Image",
            validity.with_access_mode(mode),
            AssetBorrowCounter::default(),
        )
    }

    #[test]
    fn read_and_write_admission_preserve_asset_error_text() {
        let read = core(AccessMode::Read);
        assert!(read.check_read().is_ok());
        assert_eq!(
            read.check_write().unwrap_err().to_string(),
            "Mutable access required. Use ResMut[Assets[T]] instead of Res[Assets[T]] for mutations."
        );

        let write = core(AccessMode::Write);
        assert!(write.check_read().is_ok());
        assert!(write.check_write().is_ok());
    }

    #[test]
    fn invalid_access_stays_an_access_error() {
        let error = core(AccessMode::Invalid).check_read().unwrap_err();
        assert!(matches!(
            error,
            AssetRuntimeError::Access(StorageError::InvalidAccess)
        ));
    }

    #[test]
    fn handle_identity_uses_neutral_keys_and_names() {
        let core = core(AccessMode::Read);
        assert!(core.check_handle_type(&7, "Image").is_ok());

        let error = core.check_handle_type(&9, "Mesh").unwrap_err();
        assert!(error.is_handle_type_mismatch());
        assert_eq!(
            error.to_string(),
            "Handle of type `Mesh` does not match expected type `Image`"
        );
    }
}
