//! Column capabilities for the tiled executor: typed read-only (`ColumnRef`) and
//! writable (`ColumnMut`) operand runs, plus the load/store kernels that move a
//! tile between a column and the f64 tile stack.
//!
//! Safety model: `'a` binds a column to the borrow the adapter holds, but does NOT by
//! itself prove BorrowProbe/ValidityFlag validity. The adapter must check validity and
//! run the executor within one unchecked window (see docs/safety.md). The `unsafe`
//! constructors document that obligation.

use std::marker::PhantomData;

use crate::bytecode::{FieldType, read_field_value, write_field_value};

/// Shared, read-only operand run. Broadcast is an explicit variant, not an in-band
/// stride, and it holds the constant by value so no pointer to a temporary is formed.
pub struct ColumnRef<'a> {
    kind: ColumnRefKind,
    _marker: PhantomData<&'a [u8]>,
}

#[derive(Clone, Copy)]
enum ColumnRefKind {
    /// `base` valid for `len` reads at `stride_bytes` (elem size => contiguous).
    Strided {
        base: *const u8,
        stride_bytes: isize,
        ftype: FieldType,
        len: usize,
    },
    /// The same f64 on every row. The only broadcast form; general N-D broadcast is
    /// the adapter's job (materialize).
    Broadcast(f64),
}

/// Exclusive, writable destination run. Never broadcast. A distinct type means a
/// source can never be mistaken for a destination.
pub struct ColumnMut<'a> {
    base: *mut u8,
    stride_bytes: isize,
    ftype: FieldType,
    len: usize,
    _marker: PhantomData<&'a mut [u8]>,
}

impl<'a> ColumnRef<'a> {
    /// Contiguous f64 column over a slice (safe: alignment and length come from `s`).
    pub fn from_f64_slice(s: &'a [f64]) -> Self {
        ColumnRef {
            kind: ColumnRefKind::Strided {
                base: s.as_ptr() as *const u8,
                stride_bytes: 8,
                ftype: FieldType::F64,
                len: s.len(),
            },
            _marker: PhantomData,
        }
    }

    /// Contiguous f32 column over a slice.
    pub fn from_f32_slice(s: &'a [f32]) -> Self {
        ColumnRef {
            kind: ColumnRefKind::Strided {
                base: s.as_ptr() as *const u8,
                stride_bytes: 4,
                ftype: FieldType::F32,
                len: s.len(),
            },
            _marker: PhantomData,
        }
    }

    /// Scalar broadcast: the same value on every row (no pointer).
    pub fn broadcast(value: f64) -> Self {
        ColumnRef {
            kind: ColumnRefKind::Broadcast(value),
            _marker: PhantomData,
        }
    }

    pub(crate) fn supports_native_f32(&self, count: usize) -> bool {
        match self.kind {
            ColumnRefKind::Broadcast(_) => true,
            ColumnRefKind::Strided {
                stride_bytes,
                ftype,
                len,
                ..
            } => ftype == FieldType::F32 && stride_bytes == 4 && len >= count,
        }
    }

    /// A general 1-D affine run (the View AoS case: `component_base + field.offset +
    /// row*layout_size`).
    ///
    /// # Safety
    /// `base.offset(stride_bytes * e)` must be a valid, correctly-typed, initialized
    /// `ftype` read for every `e in 0..len`, and stay valid for `'a` (the adapter holds
    /// the BorrowProbe/ValidityFlag lease and runs the executor before releasing it).
    pub unsafe fn strided_1d(
        base: *const u8,
        stride_bytes: isize,
        ftype: FieldType,
        len: usize,
    ) -> Self {
        ColumnRef {
            kind: ColumnRefKind::Strided {
                base,
                stride_bytes,
                ftype,
                len,
            },
            _marker: PhantomData,
        }
    }
}

impl<'a> ColumnMut<'a> {
    /// Contiguous f64 destination over a slice.
    pub fn from_f64_slice(s: &'a mut [f64]) -> Self {
        let len = s.len();
        ColumnMut {
            base: s.as_mut_ptr() as *mut u8,
            stride_bytes: 8,
            ftype: FieldType::F64,
            len,
            _marker: PhantomData,
        }
    }

    /// Contiguous f32 destination over a slice.
    pub fn from_f32_slice(s: &'a mut [f32]) -> Self {
        let len = s.len();
        ColumnMut {
            base: s.as_mut_ptr() as *mut u8,
            stride_bytes: 4,
            ftype: FieldType::F32,
            len,
            _marker: PhantomData,
        }
    }

    pub(crate) fn supports_native_f32(&self, count: usize) -> bool {
        self.ftype == FieldType::F32 && self.stride_bytes == 4 && self.len >= count
    }

    /// A general 1-D affine writable run (the View AoS store case).
    ///
    /// # Safety
    /// `base.offset(stride_bytes * e)` must be a valid, exclusively-owned, writable
    /// `ftype` location for every `e in 0..len`, valid for `'a`. Must not overlap any
    /// other destination, nor any source except an exact same-run in-place alias
    /// (see [`in_place_pair`]).
    pub unsafe fn strided_1d(
        base: *mut u8,
        stride_bytes: isize,
        ftype: FieldType,
        len: usize,
    ) -> Self {
        ColumnMut {
            base,
            stride_bytes,
            ftype,
            len,
            _marker: PhantomData,
        }
    }
}

/// Build a read and a write view over the SAME run, for the exact in-place alias
/// (`pos.x = f(pos.x, ...)`). The identity `(base, stride, len)` is
/// baked in, so the tile passes process the same rows in program order and a
/// store-then-read yields the per-entity value the scalar VM would.
///
/// # Safety
/// As [`ColumnRef::strided_1d`] and [`ColumnMut::strided_1d`] over one run. This is the
/// ONLY sanctioned way to alias a source and destination; partial or shifted overlap is
/// forbidden.
pub unsafe fn in_place_pair(
    base: *mut u8,
    stride_bytes: isize,
    ftype: FieldType,
    len: usize,
) -> (ColumnRef<'static>, ColumnMut<'static>) {
    (
        // SAFETY: forwarded to the caller's obligation on the shared run.
        unsafe { ColumnRef::strided_1d(base as *const u8, stride_bytes, ftype, len) },
        // SAFETY: same run, exclusive for the store phase of each tile.
        unsafe { ColumnMut::strided_1d(base, stride_bytes, ftype, len) },
    )
}

/// Load rows `[row_start, row_start+dst.len())` of `col` into the f64 tile `dst`.
/// Contiguous f32/f64 use a unit-stride (vectorizable) loop; other types and strided
/// runs widen per element through `read_field_value`, matching the scalar VM.
///
/// # Safety
/// `col` must be valid for reads over the covered rows (its construction contract).
#[inline]
pub(crate) unsafe fn load_tile_f64(col: &ColumnRef<'_>, row_start: usize, dst: &mut [f64]) {
    match col.kind {
        ColumnRefKind::Broadcast(c) => dst.fill(c),
        ColumnRefKind::Strided {
            base,
            stride_bytes,
            ftype,
            len,
        } => {
            debug_assert!(row_start + dst.len() <= len, "load past column end");
            let contiguous = stride_bytes == ftype.size_bytes() as isize;
            match (ftype, contiguous) {
                (FieldType::F64, true) => {
                    let p = base as *const f64;
                    for (l, d) in dst.iter_mut().enumerate() {
                        // SAFETY: contiguous f64 run, `row_start+l` in bounds by caller.
                        *d = unsafe { p.add(row_start + l).read_unaligned() };
                    }
                }
                (FieldType::F32, true) => {
                    let p = base as *const f32;
                    for (l, d) in dst.iter_mut().enumerate() {
                        // SAFETY: contiguous f32 run, `row_start+l` in bounds by caller.
                        *d = unsafe { p.add(row_start + l).read_unaligned() } as f64;
                    }
                }
                (FieldType::F64, false) => {
                    for (l, d) in dst.iter_mut().enumerate() {
                        // SAFETY: strided f64 element `row_start+l` valid by col contract.
                        unsafe {
                            let ptr =
                                base.offset(stride_bytes * (row_start + l) as isize) as *const f64;
                            *d = ptr.read_unaligned();
                        }
                    }
                }
                (FieldType::F32, false) => {
                    for (l, d) in dst.iter_mut().enumerate() {
                        // SAFETY: strided f32 element `row_start+l` valid by col contract.
                        unsafe {
                            let ptr =
                                base.offset(stride_bytes * (row_start + l) as isize) as *const f32;
                            *d = ptr.read_unaligned() as f64;
                        }
                    }
                }
                _ => {
                    for (l, d) in dst.iter_mut().enumerate() {
                        // SAFETY: element `row_start+l` valid; read_field_value handles ftype.
                        unsafe {
                            let ptr = base.offset(stride_bytes * (row_start + l) as isize);
                            *d = read_field_value(ptr, ftype);
                        }
                    }
                }
            }
        }
    }
}

/// Store the f64 tile `src` into rows `[row_start, row_start+src.len())` of `col`,
/// narrowing to the column's dtype. Mirror of [`load_tile_f64`].
///
/// # Safety
/// `col` must be a valid, exclusive writable run over the covered rows.
#[inline]
pub(crate) unsafe fn store_tile_f64(col: &ColumnMut<'_>, row_start: usize, src: &[f64]) {
    let ColumnMut {
        base,
        stride_bytes,
        ftype,
        len,
        ..
    } = *col;
    debug_assert!(row_start + src.len() <= len, "store past column end");
    let contiguous = stride_bytes == ftype.size_bytes() as isize;
    match (ftype, contiguous) {
        (FieldType::F64, true) => {
            let p = base as *mut f64;
            for (l, v) in src.iter().enumerate() {
                // SAFETY: contiguous f64 run, `row_start+l` in bounds by caller.
                unsafe { p.add(row_start + l).write_unaligned(*v) };
            }
        }
        (FieldType::F32, true) => {
            let p = base as *mut f32;
            for (l, v) in src.iter().enumerate() {
                // SAFETY: contiguous f32 run, `row_start+l` in bounds by caller.
                unsafe { p.add(row_start + l).write_unaligned(*v as f32) };
            }
        }
        (FieldType::F64, false) => {
            for (l, v) in src.iter().enumerate() {
                // SAFETY: strided f64 element `row_start+l` valid by col contract.
                unsafe {
                    let ptr = base.offset(stride_bytes * (row_start + l) as isize) as *mut f64;
                    ptr.write_unaligned(*v);
                }
            }
        }
        (FieldType::F32, false) => {
            for (l, v) in src.iter().enumerate() {
                // SAFETY: strided f32 element `row_start+l` valid by col contract.
                unsafe {
                    let ptr = base.offset(stride_bytes * (row_start + l) as isize) as *mut f32;
                    ptr.write_unaligned(*v as f32);
                }
            }
        }
        _ => {
            for (l, v) in src.iter().enumerate() {
                // SAFETY: element `row_start+l` valid; write_field_value handles ftype.
                unsafe {
                    let ptr = base.offset(stride_bytes * (row_start + l) as isize);
                    write_field_value(ptr, *v, ftype);
                }
            }
        }
    }
}

/// Load a broadcast or contiguous-f32 column into an f32 tile without widening.
///
/// # Safety
/// `col` must be a valid contiguous-f32 (or broadcast) read run over the covered rows.
#[inline]
pub(crate) unsafe fn load_tile_f32(col: &ColumnRef<'_>, row_start: usize, dst: &mut [f32]) {
    match col.kind {
        ColumnRefKind::Broadcast(c) => dst.fill(c as f32),
        ColumnRefKind::Strided {
            base,
            stride_bytes,
            ftype,
            len,
        } => {
            debug_assert!(row_start + dst.len() <= len, "load past column end");
            debug_assert!(
                ftype == FieldType::F32 && stride_bytes == 4,
                "native f32 load requires a contiguous f32 column"
            );
            let p = base as *const f32;
            for (l, d) in dst.iter_mut().enumerate() {
                // SAFETY: contiguous f32 run, `row_start+l` in bounds by caller.
                *d = unsafe { p.add(row_start + l).read_unaligned() };
            }
        }
    }
}

/// Store an f32 tile into a contiguous-f32 column without widening.
///
/// # Safety
/// `col` must be a valid, exclusive contiguous-f32 writable run over the covered rows.
#[inline]
pub(crate) unsafe fn store_tile_f32(col: &ColumnMut<'_>, row_start: usize, src: &[f32]) {
    let ColumnMut {
        base,
        stride_bytes,
        ftype,
        len,
        ..
    } = *col;
    debug_assert!(row_start + src.len() <= len, "store past column end");
    debug_assert!(
        ftype == FieldType::F32 && stride_bytes == 4,
        "native f32 store requires a contiguous f32 column"
    );
    let p = base as *mut f32;
    for (l, v) in src.iter().enumerate() {
        // SAFETY: contiguous f32 run, `row_start+l` in bounds by caller.
        unsafe { p.add(row_start + l).write_unaligned(*v) };
    }
}
