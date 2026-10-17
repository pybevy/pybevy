//! Owned, typed element storage. Typed enum variants keep the core free of
//! unsafe pointer/alignment handling; byte views for buffer export are an
//! adapter concern layered on top later.

use std::{ptr, sync::Arc};

use half::f16;

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
// the accessing thread. The referents are plain integers with no interior
// mutability.
/// A raw, read-only view of `len` contiguous `u16`s, gated by the owning
/// storage's probe.
#[derive(Debug, Clone, Copy)]
pub struct BorrowedU16Slice {
    ptr: *const u16,
    len: usize,
}

/// A raw, read-only view of `len` contiguous `u32`s, gated by the owning
/// storage's probe.
#[derive(Debug, Clone, Copy)]
pub struct BorrowedU32Slice {
    ptr: *const u32,
    len: usize,
}

// SAFETY: the pointer is only dereferenced behind a successful probe check on
// the owning thread, the same contract as the u8 and f32 slices above.
unsafe impl Send for BorrowedU16Slice {}
// SAFETY: as above.
unsafe impl Sync for BorrowedU16Slice {}
// SAFETY: as above.
unsafe impl Send for BorrowedU32Slice {}
// SAFETY: as above.
unsafe impl Sync for BorrowedU32Slice {}

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
    /// IEEE 754 binary16 payloads. Bits are retained verbatim so codecs and
    /// DLPack exchange preserve signed zero and NaN payloads.
    Float16(Vec<u16>),
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    Int64(Vec<i64>),
    Int32(Vec<i32>),
    Int16(Vec<i16>),
    Int8(Vec<i8>),
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
    /// A zero-copy read-only borrow of external `u16` data (a mesh index
    /// buffer), guarded by a liveness probe.
    BorrowedU16 {
        slice: BorrowedU16Slice,
        probe: Arc<dyn BorrowProbe>,
    },
    /// A zero-copy read-only borrow of external `u32` data (a mesh index
    /// buffer), guarded by a liveness probe.
    BorrowedU32 {
        slice: BorrowedU32Slice,
        probe: Arc<dyn BorrowProbe>,
    },
}

impl PartialEq for ArrayStorage {
    fn eq(&self, other: &Self) -> bool {
        use ArrayStorage::*;
        match (self, other) {
            (Float16(a), Float16(b)) => a == b,
            (Float32(a), Float32(b)) => a == b,
            (Float64(a), Float64(b)) => a == b,
            (Int64(a), Int64(b)) => a == b,
            (Int32(a), Int32(b)) => a == b,
            (Int16(a), Int16(b)) => a == b,
            (Int8(a), Int8(b)) => a == b,
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
            (
                BorrowedU16 {
                    slice: a,
                    probe: pa,
                },
                BorrowedU16 {
                    slice: b,
                    probe: pb,
                },
            ) => a.ptr == b.ptr && a.len == b.len && Arc::ptr_eq(pa, pb),
            (
                BorrowedU32 {
                    slice: a,
                    probe: pa,
                },
                BorrowedU32 {
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
            ArrayStorage::Float16(_) => ArrayDType::Float16,
            ArrayStorage::Float32(_) => ArrayDType::Float32,
            ArrayStorage::Float64(_) => ArrayDType::Float64,
            ArrayStorage::Int64(_) => ArrayDType::Int64,
            ArrayStorage::Int32(_) => ArrayDType::Int32,
            ArrayStorage::Int16(_) => ArrayDType::Int16,
            ArrayStorage::Int8(_) => ArrayDType::Int8,
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
            ArrayStorage::BorrowedU16 { .. } => ArrayDType::Uint16,
            ArrayStorage::BorrowedU32 { .. } => ArrayDType::Uint32,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ArrayStorage::Float16(v) => v.len(),
            ArrayStorage::Float32(v) => v.len(),
            ArrayStorage::Float64(v) => v.len(),
            ArrayStorage::Int64(v) => v.len(),
            ArrayStorage::Int32(v) => v.len(),
            ArrayStorage::Int16(v) => v.len(),
            ArrayStorage::Int8(v) => v.len(),
            ArrayStorage::Uint32(v) => v.len(),
            ArrayStorage::Uint16(v) => v.len(),
            ArrayStorage::Uint8(v) => v.len(),
            ArrayStorage::Bool(v) => v.len(),
            ArrayStorage::BorrowedF32 { slice, .. } => slice.len,
            ArrayStorage::BorrowedMutF32 { slice, .. } => slice.len,
            ArrayStorage::BorrowedU8 { slice, .. } => slice.len,
            ArrayStorage::BorrowedMutU8 { slice, .. } => slice.len,
            ArrayStorage::BorrowedU16 { slice, .. } => slice.len,
            ArrayStorage::BorrowedU32 { slice, .. } => slice.len,
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

    /// Return the contiguous `u8` backing slice for owned `Uint8` or either
    /// borrowed `u8` storage variant.
    ///
    /// # Safety
    /// For borrowed storage, the caller must hold a successful read claim for
    /// the whole use of the returned slice and must not re-enter Python.
    #[cfg(feature = "pyo3")]
    pub(crate) unsafe fn as_u8_contiguous_unchecked(&self) -> Option<&[u8]> {
        match self {
            ArrayStorage::Uint8(values) => Some(values),
            ArrayStorage::BorrowedU8 { slice, .. } => {
                // SAFETY: the caller upholds the probe-validity contract above.
                Some(unsafe { std::slice::from_raw_parts(slice.ptr, slice.len) })
            }
            ArrayStorage::BorrowedMutU8 { slice, .. } => {
                // SAFETY: the caller holds the read claim and the exclusive
                // lease prevents a concurrent mutable alias.
                Some(unsafe { std::slice::from_raw_parts(slice.ptr.cast_const(), slice.len) })
            }
            _ => None,
        }
    }

    /// Return the contiguous `u16`/`u32` backing slice for owned or borrowed
    /// unsigned storage of that width.
    ///
    /// # Safety
    /// As [`Self::as_u8_contiguous_unchecked`]: the caller must hold a
    /// successful read claim for the whole use of the returned slice.
    #[cfg(feature = "pyo3")]
    pub(crate) unsafe fn as_u16_contiguous_unchecked(&self) -> Option<&[u16]> {
        match self {
            ArrayStorage::Uint16(values) => Some(values),
            ArrayStorage::BorrowedU16 { slice, .. } => {
                // SAFETY: the caller upholds the probe-validity contract above.
                Some(unsafe { std::slice::from_raw_parts(slice.ptr, slice.len) })
            }
            _ => None,
        }
    }

    /// See [`Self::as_u16_contiguous_unchecked`].
    ///
    /// # Safety
    /// As that method, for `u32`.
    #[cfg(feature = "pyo3")]
    pub(crate) unsafe fn as_u32_contiguous_unchecked(&self) -> Option<&[u32]> {
        match self {
            ArrayStorage::Uint32(values) => Some(values),
            ArrayStorage::BorrowedU32 { slice, .. } => {
                // SAFETY: the caller upholds the probe-validity contract above.
                Some(unsafe { std::slice::from_raw_parts(slice.ptr, slice.len) })
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
            ArrayStorage::Float16(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Float32(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Float64(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Int64(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Int32(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Int16(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Int8(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Uint32(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Uint16(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::Uint8(values) => Some(values.as_mut_ptr().cast()),
            ArrayStorage::BorrowedMutF32 { slice, .. } => Some(slice.ptr.cast()),
            ArrayStorage::BorrowedMutU8 { slice, .. } => Some(slice.ptr),
            ArrayStorage::Bool(_)
            | ArrayStorage::BorrowedF32 { .. }
            | ArrayStorage::BorrowedU8 { .. }
            | ArrayStorage::BorrowedU16 { .. }
            | ArrayStorage::BorrowedU32 { .. } => None,
        }
    }

    /// Copy `bytes` into writable `uint8` storage at `start..start + len`.
    /// Returns `false` without writing for any other storage kind.
    ///
    /// The caller holds the backing write guard, whose probe check covers this
    /// synchronous copy.
    pub(crate) fn write_u8_range(&mut self, start: usize, bytes: &[u8]) -> bool {
        let end = start
            .checked_add(bytes.len())
            .expect("validated array range overflowed");
        match self {
            ArrayStorage::Uint8(values) => {
                values[start..end].copy_from_slice(bytes);
                true
            }
            ArrayStorage::BorrowedMutU8 { slice, .. } => {
                assert!(end <= slice.len, "index out of bounds for borrowed storage");
                // SAFETY: the range is asserted in bounds, the write guard
                // passed the probe, and the asset write lease makes this the
                // unique alias; `bytes` is a distinct shared borrow.
                unsafe {
                    ptr::copy_nonoverlapping(bytes.as_ptr(), slice.ptr.add(start), bytes.len())
                };
                true
            }
            _ => false,
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

    /// Wrap external `u16` data as a read-only borrowed storage guarded by
    /// `probe`.
    ///
    /// # Safety
    /// `ptr` must address `len` initialized contiguous `u16`s which remain
    /// valid whenever `probe.check_read()` succeeds, with no mutable alias
    /// during that operation window.
    pub unsafe fn borrowed_u16(ptr: *const u16, len: usize, probe: Arc<dyn BorrowProbe>) -> Self {
        ArrayStorage::BorrowedU16 {
            slice: BorrowedU16Slice { ptr, len },
            probe,
        }
    }

    /// Wrap external `u32` data as a read-only borrowed storage guarded by
    /// `probe`.
    ///
    /// # Safety
    /// As [`Self::borrowed_u16`], for `u32`.
    pub unsafe fn borrowed_u32(ptr: *const u32, len: usize, probe: Arc<dyn BorrowProbe>) -> Self {
        ArrayStorage::BorrowedU32 {
            slice: BorrowedU32Slice { ptr, len },
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
                | ArrayStorage::BorrowedU16 { .. }
                | ArrayStorage::BorrowedU32 { .. }
        )
    }

    /// Whether this storage is a *read-only* borrow (writes must be rejected).
    pub fn is_read_only_borrow(&self) -> bool {
        matches!(
            self,
            ArrayStorage::BorrowedF32 { .. }
                | ArrayStorage::BorrowedU8 { .. }
                | ArrayStorage::BorrowedU16 { .. }
                | ArrayStorage::BorrowedU32 { .. }
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
            | ArrayStorage::BorrowedMutU8 { probe, .. }
            | ArrayStorage::BorrowedU16 { probe, .. }
            | ArrayStorage::BorrowedU32 { probe, .. } => {
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
            ArrayStorage::BorrowedF32 { .. }
            | ArrayStorage::BorrowedU8 { .. }
            | ArrayStorage::BorrowedU16 { .. }
            | ArrayStorage::BorrowedU32 { .. } => Err(ArrayError::NotWritable),
            _ => Ok(()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Allocate `len` elements of `dtype`, each set to `value` (cast to dtype).
    pub fn filled(dtype: ArrayDType, len: usize, value: Scalar) -> ArrayResult<Self> {
        Ok(match dtype {
            ArrayDType::Float16 => ArrayStorage::Float16(try_filled_vec(
                dtype,
                len,
                f16::from_f64(value.to_f64()).to_bits(),
            )?),
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
            ArrayDType::Int16 => {
                ArrayStorage::Int16(try_filled_vec(dtype, len, value.to_i64_trunc() as i16)?)
            }
            ArrayDType::Int8 => {
                ArrayStorage::Int8(try_filled_vec(dtype, len, value.to_i64_trunc() as i8)?)
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
            ArrayStorage::Float16(v) => Scalar::F64(f64::from(f16::from_bits(v[flat]).to_f32())),
            ArrayStorage::Float32(v) => Scalar::F64(f64::from(v[flat])),
            ArrayStorage::Float64(v) => Scalar::F64(v[flat]),
            ArrayStorage::Int64(v) => Scalar::I64(v[flat]),
            ArrayStorage::Int32(v) => Scalar::I64(i64::from(v[flat])),
            ArrayStorage::Int16(v) => Scalar::I64(i64::from(v[flat])),
            ArrayStorage::Int8(v) => Scalar::I64(i64::from(v[flat])),
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
            ArrayStorage::BorrowedU16 { slice, .. } => {
                assert!(flat < slice.len, "index out of bounds for borrowed storage");
                // SAFETY: bounds are asserted and the caller checked the probe
                // for this operation on the owning thread.
                Scalar::I64(i64::from(unsafe { *slice.ptr.add(flat) }))
            }
            ArrayStorage::BorrowedU32 { slice, .. } => {
                assert!(flat < slice.len, "index out of bounds for borrowed storage");
                // SAFETY: as the u16 arm above.
                Scalar::I64(i64::from(unsafe { *slice.ptr.add(flat) }))
            }
        }
    }

    /// Append elements selected by validated flat offsets in little-endian
    /// byte order. The caller must hold a backing read guard for the whole
    /// operation; borrowed variants are dereferenced only within that guard.
    pub(crate) fn append_le_bytes(
        &self,
        offsets: impl IntoIterator<Item = usize>,
        output: &mut Vec<u8>,
    ) {
        macro_rules! append_numeric {
            ($values:expr) => {
                for offset in offsets {
                    output.extend_from_slice(&$values[offset].to_le_bytes());
                }
            };
        }

        match self {
            ArrayStorage::Float16(values) => append_numeric!(values),
            ArrayStorage::Float32(values) => append_numeric!(values),
            ArrayStorage::Float64(values) => append_numeric!(values),
            ArrayStorage::Int64(values) => append_numeric!(values),
            ArrayStorage::Int32(values) => append_numeric!(values),
            ArrayStorage::Int16(values) => append_numeric!(values),
            ArrayStorage::Int8(values) => append_numeric!(values),
            ArrayStorage::Uint32(values) => append_numeric!(values),
            ArrayStorage::Uint16(values) => append_numeric!(values),
            ArrayStorage::Uint8(values) => {
                output.extend(offsets.into_iter().map(|offset| values[offset]));
            }
            ArrayStorage::Bool(values) => {
                output.extend(offsets.into_iter().map(|offset| u8::from(values[offset])));
            }
            ArrayStorage::BorrowedF32 { slice, .. } => {
                for offset in offsets {
                    assert!(
                        offset < slice.len,
                        "validated array offset is out of bounds"
                    );
                    // SAFETY: the caller holds the read guard whose probe check
                    // covers this operation, and the validated offset is in bounds.
                    let value = unsafe { *slice.ptr.add(offset) };
                    output.extend_from_slice(&value.to_le_bytes());
                }
            }
            ArrayStorage::BorrowedMutF32 { slice, .. } => {
                for offset in offsets {
                    assert!(
                        offset < slice.len,
                        "validated array offset is out of bounds"
                    );
                    // SAFETY: as above; the mutable borrow's exclusive lease
                    // also prevents another alias from changing the referent.
                    let value = unsafe { *slice.ptr.add(offset) };
                    output.extend_from_slice(&value.to_le_bytes());
                }
            }
            ArrayStorage::BorrowedU8 { slice, .. } => {
                for offset in offsets {
                    assert!(
                        offset < slice.len,
                        "validated array offset is out of bounds"
                    );
                    // SAFETY: the read guard checked the probe and the offset.
                    output.push(unsafe { *slice.ptr.add(offset) });
                }
            }
            ArrayStorage::BorrowedMutU8 { slice, .. } => {
                for offset in offsets {
                    assert!(
                        offset < slice.len,
                        "validated array offset is out of bounds"
                    );
                    // SAFETY: the read guard and exclusive lease cover this access.
                    output.push(unsafe { *slice.ptr.add(offset) });
                }
            }
            ArrayStorage::BorrowedU16 { slice, .. } => {
                for offset in offsets {
                    assert!(
                        offset < slice.len,
                        "validated array offset is out of bounds"
                    );
                    // SAFETY: the caller holds the read guard whose probe check
                    // covers this operation, and the offset is validated.
                    let value = unsafe { *slice.ptr.add(offset) };
                    output.extend_from_slice(&value.to_le_bytes());
                }
            }
            ArrayStorage::BorrowedU32 { slice, .. } => {
                for offset in offsets {
                    assert!(
                        offset < slice.len,
                        "validated array offset is out of bounds"
                    );
                    // SAFETY: as the u16 arm above.
                    let value = unsafe { *slice.ptr.add(offset) };
                    output.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
    }

    /// Write `value` (cast to this storage's dtype) at `flat`.
    pub fn set(&mut self, flat: usize, value: Scalar) {
        match self {
            ArrayStorage::Float16(v) => {
                v[flat] = f16::from_f64(value.to_f64()).to_bits();
            }
            ArrayStorage::Float32(v) => v[flat] = value.to_f64() as f32,
            ArrayStorage::Float64(v) => v[flat] = value.to_f64(),
            ArrayStorage::Int64(v) => v[flat] = value.to_i64_trunc(),
            ArrayStorage::Int32(v) => v[flat] = value.to_i64_trunc() as i32,
            ArrayStorage::Int16(v) => v[flat] = value.to_i64_trunc() as i16,
            ArrayStorage::Int8(v) => v[flat] = value.to_i64_trunc() as i8,
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
            ArrayStorage::BorrowedU8 { .. }
            | ArrayStorage::BorrowedU16 { .. }
            | ArrayStorage::BorrowedU32 { .. } => {
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

    /// Return an owned float16 element's raw IEEE 754 payload.
    pub(crate) fn float16_bits(&self, flat: usize) -> Option<u16> {
        match self {
            ArrayStorage::Float16(values) => Some(values[flat]),
            _ => None,
        }
    }

    /// Replace an owned float16 element without decoding its payload.
    pub(crate) fn set_float16_bits(&mut self, flat: usize, bits: u16) {
        match self {
            ArrayStorage::Float16(values) => values[flat] = bits,
            _ => unreachable!("float16 bits require float16 storage"),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[test]
    fn filled_and_get_roundtrip() {
        let s = ArrayStorage::filled(ArrayDType::Float64, 3, Scalar::F64(2.5)).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s.get(1), Scalar::F64(2.5));
    }

    #[test]
    fn float_to_int_truncates_toward_zero() {
        let s = ArrayStorage::filled(ArrayDType::Int32, 1, Scalar::F64(-2.9)).unwrap();
        assert_eq!(s.get(0), Scalar::I64(-2));
    }

    #[test]
    fn nonzero_casts_to_true_bool() {
        let s = ArrayStorage::filled(ArrayDType::Bool, 1, Scalar::F64(-0.1)).unwrap();
        assert_eq!(s.get(0), Scalar::Bool(true));
        let z = ArrayStorage::filled(ArrayDType::Bool, 1, Scalar::F64(0.0)).unwrap();
        assert_eq!(z.get(0), Scalar::Bool(false));
    }

    #[test]
    fn int64_exact_beyond_f64() {
        let big = 9_007_199_254_740_993_i64; // 2^53 + 1, not exact in f64
        let mut s = ArrayStorage::zeros(ArrayDType::Int64, 1).unwrap();
        s.set(0, Scalar::I64(big));
        assert_eq!(s.get(0), Scalar::I64(big));
    }

    #[test]
    fn impossible_allocation_is_reported() {
        let err = ArrayStorage::zeros(ArrayDType::Float64, usize::MAX).unwrap_err();
        assert!(matches!(err, ArrayError::AllocationFailed { .. }));
    }

    #[derive(Debug)]
    struct ToggleProbe {
        live: AtomicBool,
        writable: bool,
    }
    impl ToggleProbe {
        fn read_only() -> Arc<Self> {
            Arc::new(Self {
                live: AtomicBool::new(true),
                writable: false,
            })
        }
        fn writable() -> Arc<Self> {
            Arc::new(Self {
                live: AtomicBool::new(true),
                writable: true,
            })
        }
    }
    impl BorrowProbe for ToggleProbe {
        fn check_read(&self) -> Result<(), String> {
            if self.live.load(Ordering::Acquire) {
                Ok(())
            } else {
                Err("expired".to_string())
            }
        }
        fn check_write(&self) -> Result<(), String> {
            if !self.writable {
                return Err("read-only".to_string());
            }
            self.check_read()
        }
    }

    fn borrowed(data: &[f32], probe: Arc<dyn BorrowProbe>) -> ArrayStorage {
        // SAFETY: `data` outlives the storage in each test, and the probe is
        // consulted before every read.
        unsafe { ArrayStorage::borrowed_f32(data.as_ptr(), data.len(), probe) }
    }

    #[test]
    fn borrowed_reads_while_live() {
        let data = [1.0f32, 2.0, 3.0];
        let probe = ToggleProbe::read_only();
        let s = borrowed(&data, probe);
        assert_eq!(s.dtype(), ArrayDType::Float32);
        assert_eq!(s.len(), 3);
        assert!(s.ensure_readable().is_ok());
        assert_eq!(s.get(1), Scalar::F64(2.0));
    }

    #[test]
    fn borrowed_ensure_readable_errs_after_expiry() {
        let data = [1.0f32, 2.0];
        let probe = ToggleProbe::read_only();
        let s = borrowed(&data, probe.clone());
        probe.live.store(false, Ordering::Release);
        assert!(matches!(
            s.ensure_readable(),
            Err(ArrayError::BorrowExpired(_))
        ));
    }

    #[test]
    #[should_panic(expected = "read-only borrowed array")]
    fn borrowed_set_panics() {
        let data = [1.0f32];
        let probe = ToggleProbe::read_only();
        let mut s = borrowed(&data, probe);
        s.set(0, Scalar::F64(9.0));
    }

    #[test]
    fn set_casts_to_storage_dtype() {
        let mut s = ArrayStorage::zeros(ArrayDType::Uint8, 1).unwrap();
        s.set(0, Scalar::I64(300)); // wraps like `as u8`
        assert_eq!(s.get(0), Scalar::I64(300 % 256));
    }

    fn borrowed_mut(data: &mut [f32], probe: Arc<dyn BorrowProbe>) -> ArrayStorage {
        // SAFETY: `data` outlives the storage and is the unique alias while the
        // returned storage is alive in each test.
        unsafe { ArrayStorage::borrowed_mut_f32(data.as_mut_ptr(), data.len(), probe) }
    }

    #[test]
    fn borrowed_mut_reads_and_writes_in_place() {
        let mut data = [1.0f32, 2.0, 3.0];
        let mut s = borrowed_mut(&mut data, ToggleProbe::writable());
        assert!(s.ensure_writable().is_ok());
        assert_eq!(s.get(0), Scalar::F64(1.0));
        s.set(0, Scalar::F64(9.0));
        assert_eq!(s.get(0), Scalar::F64(9.0));
        drop(s);
        assert_eq!(data[0], 9.0); // the write landed in the borrowed buffer
    }

    #[test]
    fn borrowed_mut_ensure_writable_errs_after_expiry() {
        let mut data = [1.0f32];
        let probe = ToggleProbe::writable();
        let s = borrowed_mut(&mut data, probe.clone());
        probe.live.store(false, Ordering::Release);
        assert!(matches!(
            s.ensure_writable(),
            Err(ArrayError::BorrowExpired(_))
        ));
        assert!(matches!(
            s.ensure_readable(),
            Err(ArrayError::BorrowExpired(_))
        ));
    }

    #[test]
    fn borrowed_mut_with_read_only_probe_rejects_write() {
        let mut data = [1.0f32];
        let s = borrowed_mut(&mut data, ToggleProbe::read_only());
        assert!(matches!(
            s.ensure_writable(),
            Err(ArrayError::BorrowExpired(_))
        ));
    }

    fn borrowed_u8(data: &[u8], probe: Arc<dyn BorrowProbe>) -> ArrayStorage {
        // SAFETY: `data` outlives the storage and the probe gates every read.
        unsafe { ArrayStorage::borrowed_u8(data.as_ptr(), data.len(), probe) }
    }

    fn borrowed_mut_u8(data: &mut [u8], probe: Arc<dyn BorrowProbe>) -> ArrayStorage {
        // SAFETY: `data` outlives the storage and is uniquely borrowed.
        unsafe { ArrayStorage::borrowed_mut_u8(data.as_mut_ptr(), data.len(), probe) }
    }

    #[test]
    fn borrowed_u8_reads_as_uint8() {
        let data = [10u8, 200, 255];
        let s = borrowed_u8(&data, ToggleProbe::read_only());
        assert_eq!(s.dtype(), ArrayDType::Uint8);
        assert_eq!(s.len(), 3);
        assert!(s.is_borrowed() && s.is_read_only_borrow());
        assert_eq!(s.get(1), Scalar::I64(200));
    }

    #[test]
    fn borrowed_u8_rejects_reads_after_expiry() {
        let data = [1u8, 2];
        let probe = ToggleProbe::read_only();
        let s = borrowed_u8(&data, probe.clone());
        probe.live.store(false, Ordering::Release);
        assert!(matches!(
            s.ensure_readable(),
            Err(ArrayError::BorrowExpired(_))
        ));
    }

    #[test]
    fn borrowed_unsigned_index_storage_is_read_only_and_expires() {
        let data_u16 = [0_u16, 2, 1];
        let data_u32 = [0_u32, 2, 1];
        let probe = ToggleProbe::read_only();
        let probe_dyn: Arc<dyn BorrowProbe> = probe.clone();
        // SAFETY: both arrays outlive their storage and the shared probe gates
        // every read.
        let u16_storage = unsafe {
            ArrayStorage::borrowed_u16(data_u16.as_ptr(), data_u16.len(), Arc::clone(&probe_dyn))
        };
        // SAFETY: as above, for the u32 array.
        let u32_storage =
            unsafe { ArrayStorage::borrowed_u32(data_u32.as_ptr(), data_u32.len(), probe_dyn) };

        assert_eq!(u16_storage.dtype(), ArrayDType::Uint16);
        assert_eq!(u32_storage.dtype(), ArrayDType::Uint32);
        assert_eq!(u16_storage.get(1), Scalar::I64(2));
        assert_eq!(u32_storage.get(2), Scalar::I64(1));
        assert!(matches!(
            u16_storage.ensure_writable(),
            Err(ArrayError::NotWritable)
        ));
        assert!(matches!(
            u32_storage.ensure_writable(),
            Err(ArrayError::NotWritable)
        ));

        probe.live.store(false, Ordering::Release);
        assert!(matches!(
            u16_storage.ensure_readable(),
            Err(ArrayError::BorrowExpired(_))
        ));
        assert!(matches!(
            u32_storage.ensure_readable(),
            Err(ArrayError::BorrowExpired(_))
        ));
    }

    #[test]
    fn borrowed_unsigned_index_storage_compares_by_source() {
        let data_u16 = [0_u16, 1, 2];
        let data_u32 = [0_u32, 1, 2];
        let probe: Arc<dyn BorrowProbe> = ToggleProbe::read_only();
        // SAFETY: both arrays outlive the storage values and the probe gates reads.
        let u16_first = unsafe {
            ArrayStorage::borrowed_u16(data_u16.as_ptr(), data_u16.len(), Arc::clone(&probe))
        };
        // SAFETY: as above; this deliberately describes the same source.
        let u16_second = unsafe {
            ArrayStorage::borrowed_u16(data_u16.as_ptr(), data_u16.len(), Arc::clone(&probe))
        };
        // SAFETY: as above, for the u32 source.
        let u32_first = unsafe {
            ArrayStorage::borrowed_u32(data_u32.as_ptr(), data_u32.len(), Arc::clone(&probe))
        };
        // SAFETY: as above; this deliberately describes the same source.
        let u32_second =
            unsafe { ArrayStorage::borrowed_u32(data_u32.as_ptr(), data_u32.len(), probe) };

        assert_eq!(u16_first, u16_second);
        assert_eq!(u32_first, u32_second);
    }

    #[test]
    fn borrowed_mut_u8_writes_in_place() {
        let mut data = [1u8, 2, 3];
        let mut s = borrowed_mut_u8(&mut data, ToggleProbe::writable());
        s.ensure_writable().unwrap();
        s.set(0, Scalar::I64(200));
        assert_eq!(s.get(0), Scalar::I64(200));
        drop(s);
        assert_eq!(data[0], 200);
    }
}
