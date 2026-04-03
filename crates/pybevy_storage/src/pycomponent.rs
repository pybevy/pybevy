//! Generic component storage supporting both owned and borrowed instances
//!
//! This module provides a unified storage mechanism for all PyBevy component types.
//! See the main crate's `pycomponent.rs` for detailed safety documentation.

use bevy::ecs::component::Component;

use crate::{
    storage_error::StorageError,
    storage_traits::BorrowableStorage,
    validity_guard::{ValidityFlag, ValidityFlagWithMode},
};

/// Generic storage for PyBevy components
///
/// Supports two modes:
/// - `Owned`: Python-created component, fully owned by Python
/// - `Borrowed`: Reference to component in Bevy's ECS storage
#[derive(Debug)]
pub struct ComponentStorage<T: Component> {
    pub inner: ComponentStorageInner<T>,
}

#[derive(Debug)]
pub enum ComponentStorageInner<T: Component> {
    /// Python-created instance, fully owned with validity tracking
    Owned {
        data: Box<T>,
        validity: ValidityFlag,
    },

    /// Borrowed reference to component in ECS storage
    Borrowed {
        ptr: *mut T,
        validity: ValidityFlagWithMode,
    },
}

// SAFETY: See main crate's pycomponent.rs for detailed safety analysis
unsafe impl<T: Component + Send> Send for ComponentStorage<T> {}
unsafe impl<T: Component + Sync> Sync for ComponentStorage<T> {}

impl<T: Component + Clone> Clone for ComponentStorage<T> {
    fn clone(&self) -> Self {
        match &self.inner {
            ComponentStorageInner::Owned { data, validity: _ } => Self {
                inner: ComponentStorageInner::Owned {
                    data: Box::new((**data).clone()),
                    validity: ValidityFlag::new_write(),
                },
            },
            ComponentStorageInner::Borrowed { ptr, validity } => Self {
                inner: ComponentStorageInner::Borrowed {
                    ptr: *ptr,
                    validity: validity.clone(),
                },
            },
        }
    }
}

impl<T: Component> Drop for ComponentStorage<T> {
    fn drop(&mut self) {
        if let ComponentStorageInner::Owned { validity, .. } = &self.inner {
            validity.set_invalid();
        }
    }
}

impl<T: Component + PartialEq> PartialEq for ComponentStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (
                ComponentStorageInner::Owned { data: a, .. },
                ComponentStorageInner::Owned { data: b, .. },
            ) => **a == **b,
            (
                ComponentStorageInner::Borrowed { ptr: a, .. },
                ComponentStorageInner::Borrowed { ptr: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

impl<T: Component> ComponentStorage<T> {
    /// Create owned component storage with validity tracking
    pub fn owned(component: T) -> Self {
        Self {
            inner: ComponentStorageInner::Owned {
                data: Box::new(component),
                validity: ValidityFlag::new_write(),
            },
        }
    }

    /// Create borrowed component storage with direct pointer to component
    ///
    /// # Safety
    /// - `ptr` must point to a valid `T` in ECS component storage
    /// - The pointer must remain valid while `validity` flag is true
    pub unsafe fn borrowed(ptr: *mut T, validity: ValidityFlagWithMode) -> Self {
        Self {
            inner: ComponentStorageInner::Borrowed { ptr, validity },
        }
    }

    /// Get immutable reference to the component, checking validity
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, StorageError> {
        self.check_valid()?;
        Ok(unsafe { &*self.as_ptr() })
    }

    /// Get mutable reference to the component, checking validity and write permission
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, StorageError> {
        self.check_valid_mut()?;
        Ok(unsafe { &mut *self.as_mut_ptr() })
    }

    #[inline(always)]
    fn as_ptr(&self) -> *const T {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => &**data as *const T,
            ComponentStorageInner::Borrowed { ptr, .. } => *ptr as *const T,
        }
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut T {
        match &mut self.inner {
            ComponentStorageInner::Owned { data, .. } => &mut **data as *mut T,
            ComponentStorageInner::Borrowed { ptr, .. } => *ptr,
        }
    }

    #[inline(always)]
    fn check_valid(&self) -> Result<(), StorageError> {
        match &self.inner {
            ComponentStorageInner::Owned { .. } => Ok(()),
            ComponentStorageInner::Borrowed { validity, .. } => validity.check(),
        }
    }

    #[inline(always)]
    fn check_valid_mut(&self) -> Result<(), StorageError> {
        match &self.inner {
            ComponentStorageInner::Owned { .. } => Ok(()),
            ComponentStorageInner::Borrowed { validity, .. } => validity.check_write(),
        }
    }

    #[allow(dead_code)]
    pub fn is_owned(&self) -> bool {
        matches!(self.inner, ComponentStorageInner::Owned { .. })
    }

    #[allow(dead_code)]
    pub fn is_borrowed(&self) -> bool {
        matches!(self.inner, ComponentStorageInner::Borrowed { .. })
    }

    /// Borrow a field from the component storage
    ///
    /// For owned storage, returns a read-only snapshot of the field.
    /// For borrowed storage, returns a borrowed pointer into the ECS data.
    pub fn borrow_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<S, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => Ok(S::snapshot(field_accessor(&**data))),
            ComponentStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                let component_ref = unsafe { &**ptr };
                let field_ref = field_accessor(component_ref);
                let field_ptr = field_ref as *const F as *mut F;
                Ok(unsafe { S::borrowed(field_ptr, validity.clone()) })
            }
        }
    }

    /// Borrow a field and wrap it in the final Python type
    pub fn borrow_field_as<F: Clone, S, W>(
        &self,
        field_accessor: impl Fn(&T) -> &F,
    ) -> Result<W, StorageError>
    where
        S: BorrowableStorage<F>,
        W: crate::storage_traits::FromBorrowedStorage<S>,
    {
        Ok(W::from_borrowed(self.borrow_field(field_accessor)?))
    }

    /// Borrow an optional field from the component storage
    ///
    /// For owned storage, returns a read-only snapshot of the field.
    /// For borrowed storage, returns a borrowed pointer into the ECS data.
    pub fn borrow_optional_field<F: Clone, S>(
        &self,
        field_accessor: impl Fn(&T) -> &Option<F>,
    ) -> Result<Option<S>, StorageError>
    where
        S: BorrowableStorage<F>,
    {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => match field_accessor(&**data) {
                Some(field_ref) => Ok(Some(S::snapshot(field_ref))),
                None => Ok(None),
            },
            ComponentStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                let component_ref = unsafe { &**ptr };
                match field_accessor(component_ref) {
                    Some(field_ref) => {
                        let field_ptr = field_ref as *const F as *mut F;
                        Ok(Some(unsafe { S::borrowed(field_ptr, validity.clone()) }))
                    }
                    None => Ok(None),
                }
            }
        }
    }
}

impl<T: Component + Clone> ComponentStorage<T> {
    /// Convert storage to owned component, consuming self
    pub fn into_owned(self) -> Result<T, StorageError> {
        match &self.inner {
            ComponentStorageInner::Owned { data, .. } => Ok((**data).clone()),
            ComponentStorageInner::Borrowed { ptr, validity } => {
                validity.check()?;
                Ok(unsafe { (**ptr).clone() })
            }
        }
    }
}
