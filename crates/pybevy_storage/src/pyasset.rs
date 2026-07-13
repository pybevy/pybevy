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

use bevy::asset::{Asset, UntypedHandle};

use crate::{
    ValidityFlagWithMode,
    borrowed::{BorrowedMut, BorrowedRef},
    storage_error::StorageError,
};

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
    BorrowedRef {
        /// Typed read-only borrow into the asset
        borrow: BorrowedRef<T>,

        /// Handle to the asset (for debugging and future use)
        #[allow(dead_code)]
        handle: UntypedHandle,
    },

    /// Mutable borrowed reference to asset in Assets<T> storage
    /// Created from `&mut T` - mutation is sound
    BorrowedMut {
        /// Typed mutable borrow into the asset
        borrow: BorrowedMut<T>,

        /// Handle to the asset (for debugging and future use)
        #[allow(dead_code)]
        handle: UntypedHandle,
    },
}

impl<T: Asset + Clone> Clone for AssetStorage<T> {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            AssetStorageInner::Owned(Some(asset)) => {
                AssetStorageInner::Owned(Some(Box::new((**asset).clone())))
            }
            AssetStorageInner::Owned(None) => AssetStorageInner::Owned(None),
            AssetStorageInner::BorrowedRef { borrow, handle } => AssetStorageInner::BorrowedRef {
                borrow: borrow.clone(),
                handle: handle.clone(),
            },
            // A cloned mutable borrow downgrades to read-only to avoid aliasing.
            AssetStorageInner::BorrowedMut { borrow, handle } => AssetStorageInner::BorrowedRef {
                borrow: borrow.clone_as_ref(),
                handle: handle.clone(),
            },
        };
        Self { inner }
    }
}

impl<T: Asset + PartialEq> PartialEq for AssetStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (AssetStorageInner::Owned(Some(a)), AssetStorageInner::Owned(Some(b))) => **a == **b,
            (AssetStorageInner::Owned(None), AssetStorageInner::Owned(None)) => true,
            (
                AssetStorageInner::BorrowedRef { borrow: a, .. },
                AssetStorageInner::BorrowedRef { borrow: b, .. },
            ) => a.as_ptr() == b.as_ptr(),
            (
                AssetStorageInner::BorrowedMut { borrow: a, .. },
                AssetStorageInner::BorrowedMut { borrow: b, .. },
            ) => a.as_ptr() == b.as_ptr(),
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
            AssetStorageInner::BorrowedRef { .. } | AssetStorageInner::BorrowedMut { .. } => {
                Err(StorageError::AssetBorrowed)
            }
        }
    }

    /// Create read-only borrowed asset storage from `&T`
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
        handle: UntypedHandle,
    ) -> Self {
        Self {
            inner: AssetStorageInner::BorrowedRef {
                // SAFETY: forwards this constructor's contract unchanged
                borrow: unsafe { BorrowedRef::new(ptr, validity.flag) },
                handle,
            },
        }
    }

    /// Create mutable borrowed asset storage from `&mut T`
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in `Assets<T>` storage, from `&mut T`
    /// - The pointer must remain valid while `validity` is non-Invalid
    pub unsafe fn borrowed_mut(
        ptr: *mut T,
        validity: ValidityFlagWithMode,
        handle: UntypedHandle,
    ) -> Self {
        Self {
            inner: AssetStorageInner::BorrowedMut {
                // SAFETY: forwards this constructor's contract unchanged
                borrow: unsafe { BorrowedMut::new(ptr, validity.flag) },
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
        match &self.inner {
            AssetStorageInner::Owned(Some(asset)) => Ok(&**asset),
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            AssetStorageInner::BorrowedRef { borrow, .. } => borrow.get(),
            AssetStorageInner::BorrowedMut { borrow, .. } => borrow.get(),
        }
    }

    /// Get mutable reference to the asset, checking validity
    ///
    /// # Errors
    /// Returns `StorageError` if the borrowed reference is no longer valid, or
    /// `AssetReadOnly` if the asset was borrowed read-only.
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        match &mut self.inner {
            AssetStorageInner::Owned(Some(asset)) => Ok(&mut **asset),
            AssetStorageInner::Owned(None) => Err(StorageError::AssetConsumed),
            AssetStorageInner::BorrowedRef { .. } => Err(StorageError::AssetReadOnly),
            AssetStorageInner::BorrowedMut { borrow, .. } => borrow.get_mut(),
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
            AssetStorageInner::BorrowedRef { .. } | AssetStorageInner::BorrowedMut { .. }
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

#[cfg(test)]
mod tests {
    use bevy::asset::{Asset, Handle};

    use super::*;
    use crate::{AccessMode, ValidityFlag, ValidityGuard};

    #[derive(Clone, Debug, PartialEq, bevy::reflect::TypePath)]
    struct TestAsset {
        value: i32,
    }

    impl bevy::asset::VisitAssetDependencies for TestAsset {
        fn visit_dependencies(&self, _visit: &mut impl FnMut(bevy::asset::UntypedAssetId)) {}
    }

    impl Asset for TestAsset {}

    /// Create a test handle for use in tests
    fn test_handle() -> bevy::asset::UntypedHandle {
        Handle::<TestAsset>::default().untyped()
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
        let handle = test_handle();

        let storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                handle,
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
        let handle = test_handle();

        let mut storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                handle,
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
        let handle = test_handle();

        let mut storage = unsafe {
            AssetStorage::borrowed_mut(
                &mut asset as *mut TestAsset,
                validity.with_access_mode(AccessMode::Write),
                handle,
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
        let handle = test_handle();

        let storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                handle,
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
        let handle = test_handle();

        let storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                handle,
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
        let handle = test_handle();

        let mut storage = unsafe {
            AssetStorage::borrowed_readonly(
                &asset as *const TestAsset,
                validity.with_access_mode(AccessMode::Read),
                handle,
            )
        };

        assert!(matches!(storage.take(), Err(StorageError::AssetBorrowed)));
    }

    #[test]
    fn test_take_on_borrowed_mut_fails() {
        let mut asset = TestAsset { value: 42 };
        let validity = ValidityFlag::new_write();
        let handle = test_handle();

        let mut storage = unsafe {
            AssetStorage::borrowed_mut(
                &mut asset as *mut TestAsset,
                validity.with_access_mode(AccessMode::Write),
                handle,
            )
        };

        assert!(matches!(storage.take(), Err(StorageError::AssetBorrowed)));
    }
}
