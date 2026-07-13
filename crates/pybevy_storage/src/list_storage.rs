//! Generic list storage supporting both owned and borrowed Vec instances
//!
//! This module provides storage for Vec<T> fields that can be accessed from
//! component fields, enabling mutations to persist back to ECS.

use crate::{
    borrowed::{BorrowedMut, BorrowedRef},
    storage_error::StorageError,
    storage_traits::BorrowableStorage,
    validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode},
};

/// Generic storage for Vec<T> fields
///
/// Supports two modes:
/// - `Owned`: Python-created Vec, stored in Box
/// - `Borrowed`: Reference to Vec field in a component
#[derive(Debug)]
pub struct ListStorage<T: Clone> {
    pub(crate) inner: ListStorageInner<T>,
}

#[derive(Debug)]
pub enum ListStorageInner<T: Clone> {
    /// Python-created Vec, stored in Box
    Owned(Box<Vec<T>>),

    /// Read-only borrow into a Vec field in a component
    BorrowedRef(BorrowedRef<Vec<T>>),

    /// Mutable borrow into a Vec field in a component
    BorrowedMut(BorrowedMut<Vec<T>>),
}

impl<T: Clone> Clone for ListStorage<T> {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            ListStorageInner::Owned(boxed) => ListStorageInner::Owned(Box::new((**boxed).clone())),
            ListStorageInner::BorrowedRef(b) => ListStorageInner::BorrowedRef(b.clone()),
            // A cloned mutable borrow downgrades to read-only to avoid aliasing.
            ListStorageInner::BorrowedMut(b) => ListStorageInner::BorrowedRef(b.clone_as_ref()),
        };
        Self { inner }
    }
}

impl<T: Clone> BorrowableStorage<Vec<T>> for ListStorage<T> {
    unsafe fn borrowed_ref(ptr: *const Vec<T>, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ListStorageInner::BorrowedRef(unsafe { BorrowedRef::new(ptr, validity) }),
        }
    }

    unsafe fn borrowed_mut(ptr: *mut Vec<T>, validity: ValidityFlag) -> Self {
        Self {
            // SAFETY: forwards this constructor's contract unchanged
            inner: ListStorageInner::BorrowedMut(unsafe { BorrowedMut::new(ptr, validity) }),
        }
    }

    fn snapshot(value: &Vec<T>) -> Self {
        Self {
            inner: ListStorageInner::Owned(Box::new(value.clone())),
        }
    }
}

impl<T: Clone> ListStorage<T> {
    /// Create owned list storage
    pub fn owned(vec: Vec<T>) -> Self {
        Self {
            inner: ListStorageInner::Owned(Box::new(vec)),
        }
    }

    /// Create borrowed list storage, choosing read vs write from the transport mode.
    ///
    /// # Safety
    /// - `ptr` must point to a valid `Vec<T>` in a component field
    /// - `ptr` must be safe to write through when `validity.access_mode()` is `Write`
    pub unsafe fn borrowed(ptr: *mut Vec<T>, validity: ValidityFlagWithMode) -> Self {
        match validity.access_mode() {
            // SAFETY: mode Write means ptr was obtained from a mutable borrow
            AccessMode::Write => unsafe {
                <Self as BorrowableStorage<Vec<T>>>::borrowed_mut(ptr, validity.flag)
            },
            // SAFETY: read-only view of the same pointer
            _ => unsafe {
                <Self as BorrowableStorage<Vec<T>>>::borrowed_ref(
                    ptr as *const Vec<T>,
                    validity.flag,
                )
            },
        }
    }

    /// Get immutable reference to the Vec, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&Vec<T>, StorageError> {
        match &self.inner {
            ListStorageInner::Owned(boxed) => Ok(&**boxed),
            ListStorageInner::BorrowedRef(b) => b.get(),
            ListStorageInner::BorrowedMut(b) => b.get(),
        }
    }

    /// Get mutable reference to the Vec, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut Vec<T>, StorageError> {
        match &mut self.inner {
            ListStorageInner::Owned(boxed) => Ok(&mut **boxed),
            ListStorageInner::BorrowedRef(_) => Err(StorageError::ReadOnly),
            ListStorageInner::BorrowedMut(b) => b.get_mut(),
        }
    }

    /// Get a clone of the Vec
    #[inline(always)]
    pub fn get(&self) -> Result<Vec<T>, StorageError> {
        Ok(self.as_ref()?.clone())
    }

    /// Get length of the Vec
    #[inline(always)]
    pub fn len(&self) -> Result<usize, StorageError> {
        Ok(self.as_ref()?.len())
    }

    /// Check if Vec is empty
    #[inline(always)]
    pub fn is_empty(&self) -> Result<bool, StorageError> {
        Ok(self.as_ref()?.is_empty())
    }
}

/// Normalize Python index (supports negative indexing)
pub fn normalize_index(index: isize, len: usize) -> Result<usize, StorageError> {
    let idx = if index < 0 {
        let pos_idx = len as isize + index;
        if pos_idx < 0 {
            return Err(StorageError::IndexOutOfRange);
        }
        pos_idx as usize
    } else {
        index as usize
    };

    if idx >= len {
        return Err(StorageError::IndexOutOfRange);
    }
    Ok(idx)
}

/// Macro to generate Python list wrappers for borrowed Vec<T> fields.
#[macro_export]
macro_rules! impl_py_list {
    // Primitive type variant - no conversion needed
    ($py_name:ident, $py_class_name:literal, $elem:ty) => {
        #[pyo3::pyclass(name = $py_class_name, skip_from_py_object)]
        #[derive(Debug, Clone)]
        pub struct $py_name {
            storage: $crate::list_storage::ListStorage<$elem>,
        }

        impl $py_name {
            pub fn from_owned(vec: Vec<$elem>) -> Self {
                Self {
                    storage: $crate::list_storage::ListStorage::owned(vec),
                }
            }

            pub fn from_borrowed(storage: $crate::list_storage::ListStorage<$elem>) -> Self {
                Self { storage }
            }
        }

        impl $crate::storage_traits::FromBorrowedStorage<$crate::list_storage::ListStorage<$elem>>
            for $py_name
        {
            fn from_borrowed(storage: $crate::list_storage::ListStorage<$elem>) -> Self {
                Self { storage }
            }
        }

        #[pyo3::pymethods]
        impl $py_name {
            #[new]
            #[pyo3(signature = (values = vec![]))]
            fn new(values: Vec<$elem>) -> Self {
                Self::from_owned(values)
            }

            fn __len__(&self) -> pyo3::PyResult<usize> {
                Ok(self.storage.len()?)
            }

            fn __getitem__(&self, index: isize) -> pyo3::PyResult<$elem> {
                let vec = self.storage.as_ref()?;
                let idx = $crate::list_storage::normalize_index(index, vec.len())?;
                Ok(vec[idx])
            }

            fn __setitem__(&mut self, index: isize, value: $elem) -> pyo3::PyResult<()> {
                let len = self.storage.as_ref()?.len();
                let idx = $crate::list_storage::normalize_index(index, len)?;
                self.storage.as_mut()?[idx] = value;
                Ok(())
            }

            fn __repr__(&self) -> pyo3::PyResult<String> {
                let vec = self.storage.as_ref()?;
                Ok(format!(concat!($py_class_name, "({:?})"), vec))
            }

            fn to_list(&self) -> pyo3::PyResult<Vec<$elem>> {
                Ok(self.storage.get()?)
            }

            fn append(&mut self, value: $elem) -> pyo3::PyResult<()> {
                self.storage.as_mut()?.push(value);
                Ok(())
            }

            #[pyo3(signature = (index = -1))]
            fn pop(&mut self, index: isize) -> pyo3::PyResult<$elem> {
                let len = self.storage.as_ref()?.len();
                if len == 0 {
                    return Err($crate::StorageError::EmptyList.into());
                }
                let idx = $crate::list_storage::normalize_index(index, len)?;
                Ok(self.storage.as_mut()?.remove(idx))
            }

            fn insert(&mut self, index: isize, value: $elem) -> pyo3::PyResult<()> {
                let len = self.storage.as_ref()?.len();
                let idx = if index < 0 {
                    let pos_idx = len as isize + index;
                    if pos_idx < 0 { 0 } else { pos_idx as usize }
                } else {
                    (index as usize).min(len)
                };
                self.storage.as_mut()?.insert(idx, value);
                Ok(())
            }

            fn clear(&mut self) -> pyo3::PyResult<()> {
                self.storage.as_mut()?.clear();
                Ok(())
            }

            fn extend(&mut self, values: Vec<$elem>) -> pyo3::PyResult<()> {
                self.storage.as_mut()?.extend(values);
                Ok(())
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validity_guard::{AccessMode, ValidityFlag, ValidityGuard};

    #[test]
    fn test_owned_storage() {
        let storage = ListStorage::owned(vec![1.0f32, 2.0, 3.0]);
        assert_eq!(storage.as_ref().unwrap(), &vec![1.0, 2.0, 3.0]);
        assert_eq!(storage.len().unwrap(), 3);
        assert!(!storage.is_empty().unwrap());
    }

    #[test]
    fn test_owned_empty() {
        let storage = ListStorage::<f32>::owned(vec![]);
        assert_eq!(storage.len().unwrap(), 0);
        assert!(storage.is_empty().unwrap());
    }

    #[test]
    fn test_owned_mutation() {
        let mut storage = ListStorage::owned(vec![1.0f32, 2.0]);
        storage.as_mut().unwrap().push(3.0);
        assert_eq!(storage.as_ref().unwrap(), &vec![1.0, 2.0, 3.0]);

        storage.as_mut().unwrap()[0] = 10.0;
        assert_eq!(storage.as_ref().unwrap()[0], 10.0);
    }

    #[test]
    fn test_borrowed_storage() {
        let mut vec = vec![1.0f32, 2.0, 3.0];
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let storage = unsafe { ListStorage::borrowed(&mut vec as *mut Vec<f32>, validity.clone()) };

        assert_eq!(storage.as_ref().unwrap(), &vec![1.0, 2.0, 3.0]);
        assert_eq!(storage.len().unwrap(), 3);
    }

    #[test]
    fn test_borrowed_mutation_persists() {
        let mut vec = vec![1.0f32, 2.0, 3.0];
        let validity = ValidityFlag::new_write().with_access_mode(AccessMode::Write);

        let mut storage =
            unsafe { ListStorage::borrowed(&mut vec as *mut Vec<f32>, validity.clone()) };

        storage.as_mut().unwrap().push(4.0);
        // Mutation persists to the original Vec
        assert_eq!(vec, vec![1.0, 2.0, 3.0, 4.0]);

        storage.as_mut().unwrap()[0] = 99.0;
        assert_eq!(vec[0], 99.0);
    }

    #[test]
    fn test_validity_enforcement() {
        let mut vec = vec![1.0f32];
        let flag = ValidityFlag::new();
        let validity = flag.with_access_mode(AccessMode::Read);

        let storage = unsafe { ListStorage::borrowed(&mut vec as *mut Vec<f32>, validity.clone()) };

        // Should work while guard is active
        {
            let _guard = ValidityGuard::new(flag.clone());
            assert!(storage.as_ref().is_ok());
            assert!(storage.len().is_ok());
            assert!(storage.is_empty().is_ok());
        }

        // Should fail after guard dropped
        assert!(storage.as_ref().is_err());
        assert!(storage.len().is_err());
        assert!(storage.is_empty().is_err());
    }

    #[test]
    fn test_write_permission_enforcement() {
        let mut vec = vec![1.0f32];
        let validity = ValidityFlag::new_read().with_access_mode(AccessMode::Read);

        let mut storage =
            unsafe { ListStorage::borrowed(&mut vec as *mut Vec<f32>, validity.clone()) };

        {
            let _guard = ValidityGuard::new(validity.flag.clone());
            // Read should work
            assert!(storage.as_ref().is_ok());
            // Write should fail (borrowed as Read)
            assert!(storage.as_mut().is_err());
        }
    }

    #[test]
    fn test_get_returns_clone() {
        let storage = ListStorage::owned(vec![1.0f32, 2.0]);
        let cloned = storage.get().unwrap();
        assert_eq!(cloned, vec![1.0, 2.0]);
    }

    #[test]
    fn test_clone_owned_creates_independent_storage() {
        let mut storage = ListStorage::owned(vec![1.0f32, 2.0]);
        let cloned = storage.clone();

        storage.as_mut().unwrap().push(3.0);
        // Clone should not be affected
        assert_eq!(cloned.as_ref().unwrap(), &vec![1.0, 2.0]);
        assert_eq!(storage.as_ref().unwrap(), &vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_normalize_index_positive() {
        assert_eq!(normalize_index(0, 3).unwrap(), 0);
        assert_eq!(normalize_index(2, 3).unwrap(), 2);
    }

    #[test]
    fn test_normalize_index_negative() {
        assert_eq!(normalize_index(-1, 3).unwrap(), 2);
        assert_eq!(normalize_index(-3, 3).unwrap(), 0);
    }

    #[test]
    fn test_normalize_index_out_of_bounds() {
        assert!(normalize_index(3, 3).is_err());
        assert!(normalize_index(-4, 3).is_err());
        assert!(normalize_index(0, 0).is_err());
    }
}
