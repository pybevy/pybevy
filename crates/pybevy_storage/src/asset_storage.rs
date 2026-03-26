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
//! - **`BorrowedReadOnly`**: Created from `Res[Assets[T]].get()`, stores `*const T`
//! - **`BorrowedMut`**: Created from `ResMut[Assets[T]].get_mut()`, stores `*mut T`

use bevy::asset::{Asset, UntypedHandle};

use crate::{ValidityFlagWithMode, storage_error::StorageError};

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
}

#[derive(Debug)]
pub(crate) enum AssetStorageInner<T: Asset> {
    /// Python-created instance, fully owned
    /// Option allows consuming the asset via take() when adding to Assets<T>
    Owned(Option<Box<T>>),

    /// Read-only borrowed reference to asset in Assets<T> storage
    /// Created from `&T` - mutation through this pointer is UB
    BorrowedReadOnly {
        /// Const pointer to asset - obtained from `&T`
        ptr: *const T,

        /// Validity tracking - prevents use after system execution
        validity: ValidityFlagWithMode,

        /// Handle to the asset (for debugging and future use)
        #[allow(dead_code)]
        handle: UntypedHandle,
    },

    /// Mutable borrowed reference to asset in Assets<T> storage
    /// Created from `&mut T` - mutation is sound
    BorrowedMut {
        /// Mutable pointer to asset - obtained from `&mut T`
        ptr: *mut T,

        /// Validity tracking with write access mode
        validity: ValidityFlagWithMode,

        /// Handle to the asset (for debugging and future use)
        #[allow(dead_code)]
        handle: UntypedHandle,
    },
}

// SAFETY: AssetStorage is Send because:
// - Box<T> is Send when T is Send
// - Raw pointer is just an address
// - ValidityFlag (Arc<AtomicBool>) is Send + Sync
// - UntypedHandle is Send
// - Validity checking prevents unsafe access
unsafe impl<T: Asset + Send> Send for AssetStorage<T> {}

// SAFETY: AssetStorage is Sync because:
// - Access is controlled by validity checking
// - ValidityFlag uses atomic operations
// - We only allow access when validity flag is true
unsafe impl<T: Asset + Sync> Sync for AssetStorage<T> {}

impl<T: Asset + Clone> Clone for AssetStorage<T> {
    fn clone(&self) -> Self {
        match &self.inner {
            AssetStorageInner::Owned(Some(asset)) => Self {
                inner: AssetStorageInner::Owned(Some(Box::new((**asset).clone()))),
            },
            AssetStorageInner::Owned(None) => Self {
                inner: AssetStorageInner::Owned(None),
            },
            AssetStorageInner::BorrowedReadOnly {
                ptr,
                validity,
                handle,
            } => Self {
                inner: AssetStorageInner::BorrowedReadOnly {
                    ptr: *ptr,
                    validity: validity.clone(),
                    handle: handle.clone(),
                },
            },
            AssetStorageInner::BorrowedMut {
                ptr,
                validity,
                handle,
            } => Self {
                inner: AssetStorageInner::BorrowedMut {
                    ptr: *ptr,
                    validity: validity.clone(),
                    handle: handle.clone(),
                },
            },
        }
    }
}

impl<T: Asset + PartialEq> PartialEq for AssetStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (AssetStorageInner::Owned(Some(a)), AssetStorageInner::Owned(Some(b))) => **a == **b,
            (AssetStorageInner::Owned(None), AssetStorageInner::Owned(None)) => true,
            (
                AssetStorageInner::BorrowedReadOnly { ptr: a, .. },
                AssetStorageInner::BorrowedReadOnly { ptr: b, .. },
            ) => a == b,
            (
                AssetStorageInner::BorrowedMut { ptr: a, .. },
                AssetStorageInner::BorrowedMut { ptr: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

impl<T: Asset> AssetStorage<T> {
    /// Create owned asset storage
    pub fn owned(asset: T) -> Self {
        Self {
            inner: AssetStorageInner::Owned(Some(Box::new(asset))),
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
        match &mut self.inner {
            AssetStorageInner::Owned(opt) => opt
                .take()
                .map(|boxed| *boxed)
                .ok_or(StorageError::AssetConsumed),
            AssetStorageInner::BorrowedReadOnly { .. } | AssetStorageInner::BorrowedMut { .. } => {
                Err(StorageError::AssetBorrowed)
            }
        }
    }

    /// Create read-only borrowed asset storage from `&T`
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in `Assets<T>` storage
    /// - The pointer must remain valid while `validity` flag is true
    /// - The returned storage MUST NOT be used for mutable access
    pub unsafe fn borrowed_readonly(
        ptr: *const T,
        validity: ValidityFlagWithMode,
        handle: UntypedHandle,
    ) -> Self {
        Self {
            inner: AssetStorageInner::BorrowedReadOnly {
                ptr,
                validity,
                handle,
            },
        }
    }

    /// Create mutable borrowed asset storage from `&mut T`
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in `Assets<T>` storage
    /// - The pointer must have been obtained from `&mut T`
    /// - The pointer must remain valid while `validity` flag is true
    pub unsafe fn borrowed_mut(
        ptr: *mut T,
        validity: ValidityFlagWithMode,
        handle: UntypedHandle,
    ) -> Self {
        Self {
            inner: AssetStorageInner::BorrowedMut {
                ptr,
                validity,
                handle,
            },
        }
    }

    /// Get immutable reference to the asset, checking validity
    ///
    /// # Errors
    /// Returns `StorageError` if the borrowed reference is no longer valid
    /// (i.e., accessed outside of system execution context)
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, StorageError> {
        self.check_valid()?;
        Ok(unsafe { &*self.as_ptr() })
    }

    /// Get mutable reference to the asset, checking validity
    ///
    /// # Errors
    /// Returns `StorageError` if the borrowed reference is no longer valid
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        self.check_write()?;
        Ok(unsafe { &mut *self.as_mut_ptr() })
    }

    /// Get raw const pointer to the asset
    ///
    /// # Safety
    /// Caller must ensure validity before dereferencing
    /// Returns null pointer if asset was consumed
    #[inline(always)]
    fn as_ptr(&self) -> *const T {
        match &self.inner {
            AssetStorageInner::Owned(Some(asset)) => &**asset as *const T,
            AssetStorageInner::Owned(None) => std::ptr::null(),
            AssetStorageInner::BorrowedReadOnly { ptr, .. } => *ptr,
            AssetStorageInner::BorrowedMut { ptr, .. } => *ptr as *const T,
        }
    }

    /// Get raw mutable pointer to the asset
    ///
    /// # Safety
    /// Caller must ensure validity before dereferencing
    /// Returns null pointer if asset was consumed or is read-only borrowed
    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut T {
        match &mut self.inner {
            AssetStorageInner::Owned(Some(asset)) => &mut **asset as *mut T,
            AssetStorageInner::Owned(None) => std::ptr::null_mut(),
            // Read-only borrowed cannot be mutated - return null
            // check_write() will catch this before we get here
            AssetStorageInner::BorrowedReadOnly { .. } => std::ptr::null_mut(),
            AssetStorageInner::BorrowedMut { ptr, .. } => *ptr,
        }
    }

    /// Check if this asset reference is still valid
    ///
    /// For owned assets, checks if not consumed.
    /// For borrowed assets, checks the validity flag.
    fn check_valid(&self) -> Result<(), StorageError> {
        match &self.inner {
            AssetStorageInner::Owned(Some(_)) => Ok(()),
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            AssetStorageInner::BorrowedReadOnly { validity, .. } => validity.check(),
            AssetStorageInner::BorrowedMut { validity, .. } => validity.check(),
        }
    }

    /// Check if the asset can be mutated (write permission)
    ///
    /// For owned assets, checks if not consumed.
    /// For borrowed assets, checks if they were obtained via ResMut and get_mut().
    fn check_write(&self) -> Result<(), StorageError> {
        match &self.inner {
            AssetStorageInner::Owned(Some(_)) => Ok(()),
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            // Read-only borrowed - always fails
            AssetStorageInner::BorrowedReadOnly { .. } => Err(StorageError::AssetReadOnly),
            // Mutable borrowed - check validity flag allows write
            AssetStorageInner::BorrowedMut { validity, .. } => validity.check_write(),
        }
    }

    /// Check if this storage contains an owned asset
    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        matches!(self.inner, AssetStorageInner::Owned(_))
    }

    /// Check if this storage contains a borrowed asset
    #[allow(dead_code)]
    pub fn is_borrowed(&self) -> bool {
        matches!(
            self.inner,
            AssetStorageInner::BorrowedReadOnly { .. } | AssetStorageInner::BorrowedMut { .. }
        )
    }
}

impl<T: Asset> AssetStorage<T> {
    /// Convert storage to owned asset, consuming self
    ///
    /// # Errors
    /// Returns error if storage contains a borrowed reference or if asset was already consumed
    pub fn into_owned(mut self) -> Result<T, StorageError> {
        self.take()
    }
}
