//! Guarded access to values held by PyBevy storage wrappers.

use std::{
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{AssetResourceReadGuard, AssetResourceWriteGuard, RevalidatingMut, RevalidatingRef};

/// Immutable access to a stored value.
///
/// Resolver-backed variants retain the guard that keeps the returned reference
/// valid for the lifetime of this value.
pub enum StorageRef<'a, T> {
    Direct(&'a T),
    Resolved {
        ptr: *const T,
        _guard: AssetResourceReadGuard,
        _lifetime: PhantomData<&'a T>,
    },
    Source(RevalidatingRef<'a, T>),
}

impl<'a, T> StorageRef<'a, T> {
    pub(crate) fn resolved(ptr: *const T, guard: AssetResourceReadGuard) -> Self {
        Self::Resolved {
            ptr,
            _guard: guard,
            _lifetime: PhantomData,
        }
    }

    /// Borrow the stored value while retaining this access guard.
    #[inline]
    pub fn reborrow(&self) -> &T {
        self
    }
}

impl<T: fmt::Debug> fmt::Debug for StorageRef<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.reborrow(), formatter)
    }
}

impl<T> Deref for StorageRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Direct(value) => value,
            // SAFETY: the resolver checked validity and the retained read guard
            // excludes mutable resolution for this reference's lifetime.
            Self::Resolved { ptr, .. } => unsafe { &**ptr },
            Self::Source(value) => value,
        }
    }
}

impl<T: PartialEq> PartialEq for StorageRef<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.reborrow() == other.reborrow()
    }
}

impl<T, U: ?Sized> PartialEq<&U> for StorageRef<'_, T>
where
    T: PartialEq<U>,
{
    fn eq(&self, other: &&U) -> bool {
        self.reborrow().eq(*other)
    }
}

/// Mutable access to a stored value.
///
/// Resolver-backed variants retain the guard that uniquely authorizes the
/// returned mutable reference.
pub enum StorageMut<'a, T> {
    Direct(&'a mut T),
    Resolved {
        ptr: *mut T,
        _guard: AssetResourceWriteGuard,
        _lifetime: PhantomData<&'a mut T>,
    },
    Source(RevalidatingMut<'a, T>),
}

impl<'a, T> StorageMut<'a, T> {
    pub(crate) fn resolved(ptr: *mut T, guard: AssetResourceWriteGuard) -> Self {
        Self::Resolved {
            ptr,
            _guard: guard,
            _lifetime: PhantomData,
        }
    }

    /// Borrow the stored value while retaining this access guard.
    #[inline]
    pub fn reborrow(&self) -> &T {
        self
    }

    /// Mutably borrow the stored value while retaining this access guard.
    #[inline]
    pub fn reborrow_mut(&mut self) -> &mut T {
        self
    }
}

impl<T: fmt::Debug> fmt::Debug for StorageMut<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.reborrow(), formatter)
    }
}

impl<T> Deref for StorageMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Direct(value) => value,
            // SAFETY: the resolver checked validity and the retained write
            // guard excludes every other resolution for this lifetime.
            Self::Resolved { ptr, .. } => unsafe { &**ptr },
            Self::Source(value) => value,
        }
    }
}

impl<T> DerefMut for StorageMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Direct(value) => value,
            // SAFETY: the resolver checked validity and the retained write
            // guard uniquely authorizes this mutable reference.
            Self::Resolved { ptr, .. } => unsafe { &mut **ptr },
            Self::Source(value) => value,
        }
    }
}
