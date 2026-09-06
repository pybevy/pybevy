//! Shared array storage and operation-scoped access guards.

use std::{
    cell::UnsafeCell,
    fmt,
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{ArrayDType, ArrayError, ArrayResult, ArrayStorage, Scalar};

const WRITE_GATE: usize = 1 << (usize::BITS - 1);
const READER_MASK: usize = WRITE_GATE - 1;

pub(crate) struct ArrayBacking {
    storage: UnsafeCell<ArrayStorage>,
    access: AtomicUsize,
    dtype: ArrayDType,
    len: usize,
}

impl fmt::Debug for ArrayBacking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArrayBacking")
            .field("dtype", &self.dtype)
            .field("len", &self.len)
            .field("access", &self.access.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

// SAFETY: `storage` is accessed only through the read/write gate below. Read
// guards may coexist, while a write guard excludes every other guard. Borrowed
// raw-pointer variants additionally check their thread-affine `BorrowProbe`
// before the guard is returned.
unsafe impl Send for ArrayBacking {}
// SAFETY: as above; shared access to the `UnsafeCell` is admitted only through
// operation guards, and mutation requires the unique write claim.
unsafe impl Sync for ArrayBacking {}

impl ArrayBacking {
    pub(crate) fn new(storage: ArrayStorage) -> Arc<Self> {
        let dtype = storage.dtype();
        let len = storage.len();
        Arc::new(Self {
            storage: UnsafeCell::new(storage),
            access: AtomicUsize::new(0),
            dtype,
            len,
        })
    }

    pub(crate) fn dtype(&self) -> ArrayDType {
        self.dtype
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn try_read(self: &Arc<Self>) -> ArrayResult<ArrayReadGuard> {
        let mut current = self.access.load(Ordering::Acquire);
        loop {
            if current & WRITE_GATE != 0 || current & READER_MASK == READER_MASK {
                return Err(ArrayError::AccessConflict);
            }
            match self.access.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let guard = ArrayReadGuard {
                        backing: self.clone(),
                    };
                    if let Err(error) = guard.ensure_readable() {
                        drop(guard);
                        return Err(error);
                    }
                    return Ok(guard);
                }
                Err(actual) => current = actual,
            }
        }
    }

    pub(crate) fn try_write(self: &Arc<Self>) -> ArrayResult<ArrayWriteGuard> {
        self.access
            .compare_exchange(0, WRITE_GATE, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| ArrayError::AccessConflict)?;
        let guard = ArrayWriteGuard {
            backing: self.clone(),
        };
        if let Err(error) = guard.ensure_writable() {
            drop(guard);
            return Err(error);
        }
        Ok(guard)
    }
}

pub(crate) struct ArrayReadGuard {
    backing: Arc<ArrayBacking>,
}

impl ArrayReadGuard {
    #[cfg(feature = "pyo3")]
    pub(crate) fn f32_contiguous(&self) -> Option<&[f32]> {
        // SAFETY: this guard owns a successful read claim for the lifetime of
        // the returned slice. Its caller must finish before Python re-entry.
        unsafe { self.deref().as_f32_contiguous_unchecked() }
    }

    #[cfg(feature = "pyo3")]
    pub(crate) fn u8_contiguous(&self) -> Option<&[u8]> {
        // SAFETY: this guard owns a successful read claim for the lifetime of
        // the returned slice. Its caller must finish before Python re-entry.
        unsafe { self.deref().as_u8_contiguous_unchecked() }
    }
}

impl fmt::Debug for ArrayReadGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArrayReadGuard").finish_non_exhaustive()
    }
}

impl Deref for ArrayReadGuard {
    type Target = ArrayStorage;

    fn deref(&self) -> &Self::Target {
        // SAFETY: construction acquired one read claim, which excludes writers.
        unsafe { &*self.backing.storage.get() }
    }
}

impl Drop for ArrayReadGuard {
    fn drop(&mut self) {
        let previous = self.backing.access.fetch_sub(1, Ordering::Release);
        debug_assert!(previous & READER_MASK > 0 && previous & WRITE_GATE == 0);
    }
}

pub(crate) struct ArrayWriteGuard {
    backing: Arc<ArrayBacking>,
}

impl ArrayWriteGuard {
    pub(crate) fn set(&mut self, flat: usize, value: Scalar) {
        // SAFETY: construction acquired the exclusive write claim.
        unsafe { &mut *self.backing.storage.get() }.set(flat, value);
    }

    #[cfg(feature = "pyo3")]
    pub(crate) fn as_mut_contiguous_ptr(&mut self) -> Option<*mut u8> {
        // SAFETY: construction acquired the exclusive write claim.
        unsafe { &mut *self.backing.storage.get() }.as_mut_contiguous_ptr()
    }
}

impl fmt::Debug for ArrayWriteGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArrayWriteGuard").finish_non_exhaustive()
    }
}

impl Deref for ArrayWriteGuard {
    type Target = ArrayStorage;

    fn deref(&self) -> &Self::Target {
        // SAFETY: construction acquired the exclusive write claim.
        unsafe { &*self.backing.storage.get() }
    }
}

impl Drop for ArrayWriteGuard {
    fn drop(&mut self) {
        let previous = self.backing.access.swap(0, Ordering::Release);
        debug_assert_eq!(previous, WRITE_GATE);
    }
}
