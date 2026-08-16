//! Owned, typed element storage. Typed enum variants keep the core free of
//! unsafe pointer/alignment handling; byte views for buffer export are an
//! adapter concern layered on top later.

use std::sync::Arc;

use crate::{
    dtype::ArrayDType,
    error::{ArrayError, ArrayResult},
    scalar::Scalar,
};

fn try_filled_vec<T: Clone>(dtype: ArrayDType, len: usize, value: T) -> ArrayResult<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| ArrayError::AllocationFailed {
            dtype,
            elements: len,
        })?;
    values.resize(len, value);
    Ok(values)
}

/// Liveness probe for borrowed storage. The concrete implementation lives
/// up-stack (where interpreter/asset validity types are available) so this
/// crate stays dependency-free. `check_read` returns `Ok` while the borrowed
/// data is safe to read on the current thread, and `Err(reason)` once it is not
/// (owning system finished, or cross-thread access).
///
/// An implementation that retains an interpreter object may be stored in a
/// shared backing only when that owner cannot reach the returned array. A
/// cyclic owner requires backend-specific, independently traversed references
/// instead of sharing one opaque interpreter reference through this trait.
pub trait BorrowProbe: Send + Sync + std::fmt::Debug {
    fn check_read(&self) -> Result<(), String>;
    /// Whether the borrowed data may be *written* on the current thread. The
    /// default rejects (read-only borrows); mutable probes override it.
    fn check_write(&self) -> Result<(), String> {
        Err("borrowed array is read-only".to_string())
    }
}

/// A raw, read-only view of `len` contiguous `f32`s. Only dereferenced from
/// [`ArrayStorage::get`], and only after the owning storage's probe has been
/// checked for the current operation (see [`ArrayStorage::ensure_readable`]).
#[derive(Debug, Clone, Copy)]
pub struct BorrowedF32Slice {
    ptr: *const f32,
    len: usize,
}

/// A raw, mutable view of `len` contiguous `f32`s. Dereferenced for reads in
/// [`ArrayStorage::get`] and writes in [`ArrayStorage::set`], each only after
/// the owning storage's probe passes for the current operation.
#[derive(Debug, Clone, Copy)]
pub struct BorrowedMutF32Slice {
    ptr: *mut f32,
    len: usize,
}

/// A raw, read-only view of `len` contiguous `u8`s. It is dereferenced only
/// after the owning storage's probe has been checked for the current operation.
#[derive(Debug, Clone, Copy)]
pub struct BorrowedU8Slice {
    ptr: *const u8,
    len: usize,
}

/// A raw, mutable view of `len` contiguous `u8`s. Reads and writes are both
/// gated by the owning storage's probe.
#[derive(Debug, Clone, Copy)]
pub struct BorrowedMutU8Slice {
    ptr: *mut u8,
    len: usize,
}

// SAFETY: the pointer is only read (never written) and only after the owning
// storage's `BorrowProbe::check_read` succeeds on the accessing thread, which is
// thread-affine for asset-borrowed data and gated by a live view counter for
// owned data. The referent is plain `f32` with no interior mutability.
unsafe impl Send for BorrowedF32Slice {}
// SAFETY: see the `Send` impl; access is shared-immutable while the probe holds.
unsafe impl Sync for BorrowedF32Slice {}
// SAFETY: a mutable borrow is created only when the asset has zero live views and
// then holds an exclusive write count that blocks every other view and `as_mut`,
// so this pointer is the unique alias. Reads/writes are gated per operation by
// the probe (thread-affine for asset borrows).
unsafe impl Send for BorrowedMutF32Slice {}
// SAFETY: see the `Send` impl; the write count guarantees no concurrent aliases.
unsafe impl Sync for BorrowedMutF32Slice {}

// SAFETY: the pointer is read only after `BorrowProbe::check_read` succeeds on
// the accessing thread. The referent is plain `u8` with no interior mutability.
unsafe impl Send for BorrowedU8Slice {}
// SAFETY: see the `Send` impl; access is shared-immutable while the probe holds.
unsafe impl Sync for BorrowedU8Slice {}
// SAFETY: construction requires an exclusive lease which blocks all aliases;
// every operation additionally checks the thread-affine probe.
unsafe impl Send for BorrowedMutU8Slice {}
// SAFETY: see the `Send` impl; the exclusive lease prevents concurrent aliases.
unsafe impl Sync for BorrowedMutU8Slice {}

#[derive(Debug)]
pub enum ArrayStorage {
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    Int64(Vec<i64>),
    Int32(Vec<i32>),
    Uint32(Vec<u32>),
    Uint16(Vec<u16>),
    Uint8(Vec<u8>),
    Bool(Vec<bool>),
    /// A zero-copy read-only borrow of external `f32` data (e.g. mesh vertex
    /// attributes), guarded by a liveness probe.
    BorrowedF32 {
        slice: BorrowedF32Slice,
        probe: Arc<dyn BorrowProbe>,
    },
    /// A zero-copy in-place *mutable* borrow of external `f32` data, guarded by a
    /// probe with an exclusive write count.
    BorrowedMutF32 {
        slice: BorrowedMutF32Slice,
        probe: Arc<dyn BorrowProbe>,
    },
    /// A zero-copy read-only borrow of external `u8` data, guarded by a
    /// liveness probe.
    BorrowedU8 {
        slice: BorrowedU8Slice,
        probe: Arc<dyn BorrowProbe>,
    },
    /// A zero-copy mutable borrow of external `u8` data, guarded by a probe
    /// holding an exclusive write lease.
    BorrowedMutU8 {
        slice: BorrowedMutU8Slice,
        probe: Arc<dyn BorrowProbe>,
    },
}

impl PartialEq for ArrayStorage {
    fn eq(&self, other: &Self) -> bool {
        use ArrayStorage::*;
        match (self, other) {
            (Float32(a), Float32(b)) => a == b,
            (Float64(a), Float64(b)) => a == b,
            (Int64(a), Int64(b)) => a == b,
            (Int32(a), Int32(b)) => a == b,
            (Uint32(a), Uint32(b)) => a == b,
            (Uint16(a), Uint16(b)) => a == b,
            (Uint8(a), Uint8(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (
                BorrowedF32 {
                    slice: a,
                    probe: pa,
                },
                BorrowedF32 {
                    slice: b,
                    probe: pb,
                },
            ) => a.ptr == b.ptr && a.len == b.len && Arc::ptr_eq(pa, pb),
            (
                BorrowedMutF32 {
                    slice: a,
                    probe: pa,
                },
                BorrowedMutF32 {
                    slice: b,
                    probe: pb,
                },
            ) => a.ptr == b.ptr && a.len == b.len && Arc::ptr_eq(pa, pb),
            (
                BorrowedU8 {
                    slice: a,
                    probe: pa,
                },
                BorrowedU8 {
                    slice: b,
                    probe: pb,
                },
            ) => a.ptr == b.ptr && a.len == b.len && Arc::ptr_eq(pa, pb),
            (
                BorrowedMutU8 {
                    slice: a,
                    probe: pa,
                },
                BorrowedMutU8 {
                    slice: b,
                    probe: pb,
                },
            ) => a.ptr == b.ptr && a.len == b.len && Arc::ptr_eq(pa, pb),
            _ => false,
        }
    }
}

impl ArrayStorage {
    pub fn dtype(&self) -> ArrayDType {
        match self {
            ArrayStorage::Float32(_) => ArrayDType::Float32,
            ArrayStorage::Float64(_) => ArrayDType::Float64,
            ArrayStorage::Int64(_) => ArrayDType::Int64,
            ArrayStorage::Int32(_) => ArrayDType::Int32,
            ArrayStorage::Uint32(_) => ArrayDType::Uint32,
            ArrayStorage::Uint16(_) => ArrayDType::Uint16,
            ArrayStorage::Uint8(_) => ArrayDType::Uint8,
            ArrayStorage::Bool(_) => ArrayDType::Bool,
            ArrayStorage::BorrowedF32 { .. } | ArrayStorage::BorrowedMutF32 { .. } => {
                ArrayDType::Float32
            }
            ArrayStorage::BorrowedU8 { .. } | ArrayStorage::BorrowedMutU8 { .. } => {
                ArrayDType::Uint8
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ArrayStorage::Float32(v) => v.len(),
            ArrayStorage::Float64(v) => v.len(),
            ArrayStorage::Int64(v) => v.len(),
            ArrayStorage::Int32(v) => v.len(),
            ArrayStorage::Uint32(v) => v.len(),
            ArrayStorage::Uint16(v) => v.len(),
            ArrayStorage::Uint8(v) => v.len(),
            ArrayStorage::Bool(v) => v.len(),
            ArrayStorage::BorrowedF32 { slice, .. } => slice.len,
            ArrayStorage::BorrowedMutF32 { slice, .. } => slice.len,
            ArrayStorage::BorrowedU8 { slice, .. } => slice.len,
            ArrayStorage::BorrowedMutU8 { slice, .. } => slice.len,
        }
    }

    /// Return the contiguous `f32` backing slice for owned `Float32` or either
    /// borrowed `f32` storage variant.
    ///
    /// # Safety
    /// For borrowed storage, the caller must first pass [`Self::ensure_readable`]
    /// on the current thread and must finish using the returned slice before any
    /// Python re-entry, world mutation, or other operation that could invalidate
    /// the borrow. This is the same check-and-use window required by [`Self::get`].
    #[cfg(feature = "numeric")]
    pub(crate) unsafe fn as_f32_contiguous_unchecked(&self) -> Option<&[f32]> {
        match self {
            ArrayStorage::Float32(values) => Some(values),
            ArrayStorage::BorrowedF32 { slice, .. } => {
                // SAFETY: the caller upholds the probe-validity contract above.
                Some(unsafe { std::slice::from_raw_parts(slice.ptr, slice.len) })
            }
            ArrayStorage::BorrowedMutF32 { slice, .. } => {
                // SAFETY: the caller upholds the probe-validity and exclusive-lease
                // contract above; this operation creates only a shared read.
                Some(unsafe { std::slice::from_raw_parts(slice.ptr.cast_const(), slice.len) })
            }
            _ => None,
        }
    }

    /// Return the base pointer for writable, byte-addressable contiguous
    /// storage. Bit-packed boolean storage and read-only borrows have no such
    /// pointer.
    ///
    /// The caller must retain a backing write guard and use the pointer only
    /// for a synchronous operation that cannot re-enter Python or otherwise
    /// invalidate a borrow.
    #[cfg(feature = "pyo3")]
    pub(crate) fn as_mut_contiguous_ptr(&mut self) -> Option<*mut u8> {
        match self {
            ArrayStorage::Float32(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Float64(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Int64(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Int32(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Uint32(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Uint16(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Uint8(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::BorrowedMutF32 { slice, .. } => Some(slice.ptr.cast()),
            ArrayStorage::BorrowedMutU8 { slice, .. } => Some(slice.ptr),
            ArrayStorage::Bool(_)
            | ArrayStorage::BorrowedF32 { .. }
            | ArrayStorage::BorrowedU8 { .. } => None,
        }
    }

    /// Wrap external `f32` data as a read-only borrowed storage guarded by
    /// `probe`.
    ///
    /// # Safety
    /// `ptr` must point to `len` initialized, contiguous `f32`s that remain
    /// valid to read on the accessing thread for as long as `probe.check_read()`
    /// returns `Ok`, and no mutable alias to that memory may exist during any
    /// window in which `check_read()` returns `Ok`.
    pub unsafe fn borrowed_f32(ptr: *const f32, len: usize, probe: Arc<dyn BorrowProbe>) -> Self {
        ArrayStorage::BorrowedF32 {
            slice: BorrowedF32Slice { ptr, len },
            probe,
        }
    }

    /// Wrap external `f32` data as an in-place mutable borrowed storage guarded
    /// by `probe` (which must permit writes via `check_write`).
    ///
    /// # Safety
    /// `ptr` must point to `len` initialized, contiguous `f32`s. While
    /// `probe.check_read()`/`check_write()` return `Ok`, this must be the unique
    /// alias to that memory on the accessing thread (the caller holds an
    /// exclusive write count that blocks all other views).
    pub unsafe fn borrowed_mut_f32(ptr: *mut f32, len: usize, probe: Arc<dyn BorrowProbe>) -> Self {
        ArrayStorage::BorrowedMutF32 {
            slice: BorrowedMutF32Slice { ptr, len },
            probe,
        }
    }

    /// Wrap external `u8` data as a read-only borrowed storage guarded by
    /// `probe`.
    ///
    /// # Safety
    /// `ptr` must address `len` initialized contiguous bytes which remain valid
    /// whenever `probe.check_read()` succeeds, with no mutable alias during that
    /// operation window.
    pub unsafe fn borrowed_u8(ptr: *const u8, len: usize, probe: Arc<dyn BorrowProbe>) -> Self {
        ArrayStorage::BorrowedU8 {
            slice: BorrowedU8Slice { ptr, len },
            probe,
        }
    }

    /// Wrap external `u8` data as a mutable borrowed storage guarded by
    /// `probe`.
    ///
    /// # Safety
    /// `ptr` must address `len` initialized contiguous bytes and be the unique
    /// alias whenever `probe.check_read()` or `check_write()` succeeds.
    pub unsafe fn borrowed_mut_u8(ptr: *mut u8, len: usize, probe: Arc<dyn BorrowProbe>) -> Self {
        ArrayStorage::BorrowedMutU8 {
            slice: BorrowedMutU8Slice { ptr, len },
            probe,
        }
    }

    /// Whether this storage aliases external data (vs. owning it).
    pub fn is_borrowed(&self) -> bool {
        matches!(
            self,
            ArrayStorage::BorrowedF32 { .. }
                | ArrayStorage::BorrowedMutF32 { .. }
                | ArrayStorage::BorrowedU8 { .. }
                | ArrayStorage::BorrowedMutU8 { .. }
        )
    }

    /// Whether this storage is a *read-only* borrow (writes must be rejected).
    pub fn is_read_only_borrow(&self) -> bool {
        matches!(
            self,
            ArrayStorage::BorrowedF32 { .. } | ArrayStorage::BorrowedU8 { .. }
        )
    }

    /// Verify the storage is safe to read for the current operation. Owned
    /// storage is always readable; borrowed storage consults its probe. Callers
    /// invoke this once at the start of each read operation; within that
    /// operation, on the same thread, validity cannot change (the only
    /// invalidator of an asset borrow runs on the owning thread, which is the
    /// thread executing this operation).
    pub fn ensure_readable(&self) -> Result<(), ArrayError> {
        match self {
            ArrayStorage::BorrowedF32 { probe, .. }
            | ArrayStorage::BorrowedMutF32 { probe, .. }
            | ArrayStorage::BorrowedU8 { probe, .. }
            | ArrayStorage::BorrowedMutU8 { probe, .. } => {
                probe.check_read().map_err(ArrayError::BorrowExpired)
            }
            _ => Ok(()),
        }
    }

    /// Verify the storage is safe to *write* for the current operation. Owned
    /// storage always is; a mutable borrow consults its probe; a read-only
    /// borrow rejects.
    pub fn ensure_writable(&self) -> Result<(), ArrayError> {
        match self {
            ArrayStorage::BorrowedMutF32 { probe, .. }
            | ArrayStorage::BorrowedMutU8 { probe, .. } => {
                probe.check_write().map_err(ArrayError::BorrowExpired)
            }
            ArrayStorage::BorrowedF32 { .. } | ArrayStorage::BorrowedU8 { .. } => {
                Err(ArrayError::NotWritable)
            }
            _ => Ok(()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Allocate `len` elements of `dtype`, each set to `value` (cast to dtype).
    pub fn filled(dtype: ArrayDType, len: usize, value: Scalar) -> ArrayResult<Self> {
        Ok(match dtype {
            ArrayDType::Float32 => {
                ArrayStorage::Float32(try_filled_vec(dtype, len, value.to_f64() as f32)?)
            }
            ArrayDType::Float64 => {
                ArrayStorage::Float64(try_filled_vec(dtype, len, value.to_f64())?)
            }
            ArrayDType::Int64 => {
                ArrayStorage::Int64(try_filled_vec(dtype, len, value.to_i64_trunc())?)
            }
            ArrayDType::Int32 => {
                ArrayStorage::Int32(try_filled_vec(dtype, len, value.to_i64_trunc() as i32)?)
            }
            ArrayDType::Uint32 => {
                ArrayStorage::Uint32(try_filled_vec(dtype, len, value.to_i64_trunc() as u32)?)
            }
            ArrayDType::Uint16 => {
                ArrayStorage::Uint16(try_filled_vec(dtype, len, value.to_i64_trunc() as u16)?)
            }
            ArrayDType::Uint8 => {
                ArrayStorage::Uint8(try_filled_vec(dtype, len, value.to_i64_trunc() as u8)?)
            }
            ArrayDType::Bool => ArrayStorage::Bool(try_filled_vec(dtype, len, value.to_bool())?),
        })
    }

    /// Allocate `len` zero-valued elements of `dtype`.
    pub fn zeros(dtype: ArrayDType, len: usize) -> ArrayResult<Self> {
        ArrayStorage::filled(dtype, len, Scalar::I64(0))
    }

    /// Read the element at `flat` as a neutral scalar.
    ///
    /// Panics if `flat` is out of range; callers plan offsets through
    /// [`crate::shape::Layout`], which bounds every offset to storage length.
    pub fn get(&self, flat: usize) -> Scalar {
        match self {
            ArrayStorage::Float32(v) => Scalar::F64(f64::from(v[flat])),
            ArrayStorage::Float64(v) => Scalar::F64(v[flat]),
            ArrayStorage::Int64(v) => Scalar::I64(v[flat]),
            ArrayStorage::Int32(v) => Scalar::I64(i64::from(v[flat])),
            ArrayStorage::Uint32(v) => Scalar::I64(i64::from(v[flat])),
            ArrayStorage::Uint16(v) => Scalar::I64(i64::from(v[flat])),
            ArrayStorage::Uint8(v) => Scalar::I64(i64::from(v[flat])),
            ArrayStorage::Bool(v) => Scalar::Bool(v[flat]),
            ArrayStorage::BorrowedF32 { slice, .. } => {
                assert!(flat < slice.len, "index out of bounds for borrowed storage");
                // SAFETY: `flat < slice.len` is asserted, and the caller checked
                // `ensure_readable()` for this operation on this thread, so the
                // probe holds for the operation's duration (thread-affine
                // invalidation). The referent is an initialized `f32`.
                let value = unsafe { *slice.ptr.add(flat) };
                Scalar::F64(f64::from(value))
            }
            ArrayStorage::BorrowedMutF32 { slice, .. } => {
                assert!(flat < slice.len, "index out of bounds for borrowed storage");
                // SAFETY: as the read arm above; the write count guarantees this
                // is the unique alias, so a shared read is sound.
                let value = unsafe { *slice.ptr.add(flat) };
                Scalar::F64(f64::from(value))
            }
            ArrayStorage::BorrowedU8 { slice, .. } => {
                assert!(flat < slice.len, "index out of bounds for borrowed storage");
                // SAFETY: bounds are asserted and the caller checked the probe
                // for this operation on the owning thread.
                Scalar::I64(i64::from(unsafe { *slice.ptr.add(flat) }))
            }
            ArrayStorage::BorrowedMutU8 { slice, .. } => {
                assert!(flat < slice.len, "index out of bounds for borrowed storage");
                // SAFETY: as above; the exclusive lease also prevents aliases.
                Scalar::I64(i64::from(unsafe { *slice.ptr.add(flat) }))
            }
        }
    }

    /// Write `value` (cast to this storage's dtype) at `flat`.
    pub fn set(&mut self, flat: usize, value: Scalar) {
        match self {
            ArrayStorage::Float32(v) => v[flat] = value.to_f64() as f32,
            ArrayStorage::Float64(v) => v[flat] = value.to_f64(),
            ArrayStorage::Int64(v) => v[flat] = value.to_i64_trunc(),
            ArrayStorage::Int32(v) => v[flat] = value.to_i64_trunc() as i32,
            ArrayStorage::Uint32(v) => v[flat] = value.to_i64_trunc() as u32,
            ArrayStorage::Uint16(v) => v[flat] = value.to_i64_trunc() as u16,
            ArrayStorage::Uint8(v) => v[flat] = value.to_i64_trunc() as u8,
            ArrayStorage::Bool(v) => v[flat] = value.to_bool(),
            ArrayStorage::BorrowedF32 { .. } => {
                // Read-only borrow: gated by `writable = false` and the backing's
                // write-guard acquisition.
                unreachable!("write to a read-only borrowed array");
            }
            ArrayStorage::BorrowedMutF32 { slice, .. } => {
                assert!(flat < slice.len, "index out of bounds for borrowed storage");
                // SAFETY: `flat < slice.len` asserted; the backing write guard
                // passed the probe's `check_write` on its owning thread, and
                // the exclusive asset claim makes this the unique alias.
                unsafe { *slice.ptr.add(flat) = value.to_f64() as f32 };
            }
            ArrayStorage::BorrowedU8 { .. } => {
                unreachable!("write to a read-only borrowed array");
            }
            ArrayStorage::BorrowedMutU8 { slice, .. } => {
                assert!(flat < slice.len, "index out of bounds for borrowed storage");
                // SAFETY: bounds are asserted, the backing write guard passed
                // the probe, and the asset write lease makes this unique.
                unsafe { *slice.ptr.add(flat) = value.to_i64_trunc() as u8 };
            }
        }
    }
}
