//! PyO3 adapter for PyBevy's bounded array API.
//!
//! Exposes a native `Array` pyclass over [`crate::DenseArrayCore`] plus
//! constructors, element-wise functions, reductions, comparison
//! helpers, and dtype constants. Float element-wise math runs on the safe dense VM
//! (`pybevy_bytecodevm::dense`); comparisons and reductions are exact per dtype.
//!
//! The same bounded implementation is exposed on every Python backend as
//! `pybevy.array`; full NumPy remains available separately as `import numpy`.

mod array;
mod convert;
mod dtype;
mod funcs;
mod hint;
mod kernels;
mod lens;

use std::sync::Arc;

pub use array::PyArray;
pub use dtype::PyDType;
pub use lens::ArrayLens;
use pyo3::{prelude::*, types::PyModule};

use crate::{ArrayDType, ArrayStorage, BorrowProbe, DenseArrayCore};

/// Wrap owned `float32` data as a read-only bounded array of `shape`. Engine
/// APIs (mesh, image) use this to hand back attribute snapshots that carry the
/// bounded-array surface. Writing to the result raises because it is a copy.
pub fn read_only_f32(data: Vec<f32>, shape: &[usize]) -> PyResult<PyArray> {
    let core = crate::kernels::read_only_f32_core(data, shape).map_err(kernels::map_kernel_err)?;
    Ok(PyArray { core })
}

/// Wrap external `float32` data as a read-only, zero-copy bounded array guarded
/// by `probe`. The array is read-only (writes raise) and every operation checks
/// `probe.check_read()` first, so an escaped reference raises a clean error once
/// the borrow expires instead of reading freed memory.
///
/// # Safety
/// `ptr` must point to `len` initialized, contiguous `f32`s valid to read on the
/// accessing thread whenever `probe.check_read()` returns `Ok`, with no mutable
/// alias during any such window (see [`ArrayStorage::borrowed_f32`]).
pub unsafe fn borrowed_read_only_f32(
    ptr: *const f32,
    len: usize,
    shape: &[usize],
    probe: Arc<dyn BorrowProbe>,
) -> PyResult<PyArray> {
    // SAFETY: forwarded to the caller's contract above.
    let storage = unsafe { ArrayStorage::borrowed_f32(ptr, len, probe) };
    // `from_storage` marks a read-only borrow frozen.
    let core = DenseArrayCore::from_storage(storage, shape).map_err(kernels::map_array_err)?;
    Ok(PyArray { core })
}

/// Wrap external `float32` data as an in-place *mutable* zero-copy bounded array
/// guarded by `probe`. Writes land directly in the borrowed buffer; each read
/// and write first checks the probe (so an escaped or closed reference raises).
///
/// # Safety
/// While `probe.check_read()`/`check_write()` return `Ok`, `ptr` must be the
/// unique alias to `len` initialized contiguous `f32`s on the accessing thread
/// (see [`ArrayStorage::borrowed_mut_f32`]).
pub unsafe fn borrowed_mut_f32(
    ptr: *mut f32,
    len: usize,
    shape: &[usize],
    probe: Arc<dyn BorrowProbe>,
) -> PyResult<PyArray> {
    // SAFETY: forwarded to the caller's contract above.
    let storage = unsafe { ArrayStorage::borrowed_mut_f32(ptr, len, probe) };
    let core = DenseArrayCore::from_storage(storage, shape).map_err(kernels::map_array_err)?;
    Ok(PyArray { core })
}

/// Wrap external `u8` data as a read-only zero-copy bounded array guarded by
/// `probe`.
///
/// # Safety
/// `ptr` must address `len` initialized contiguous bytes which remain valid
/// whenever `probe.check_read()` succeeds, with no mutable alias during that
/// operation window.
pub unsafe fn borrowed_read_only_u8(
    ptr: *const u8,
    len: usize,
    shape: &[usize],
    probe: Arc<dyn BorrowProbe>,
) -> PyResult<PyArray> {
    // SAFETY: forwarded to the caller's contract above.
    let storage = unsafe { ArrayStorage::borrowed_u8(ptr, len, probe) };
    let core = DenseArrayCore::from_storage(storage, shape).map_err(kernels::map_array_err)?;
    Ok(PyArray { core })
}

/// Wrap external `u8` data as an in-place mutable zero-copy bounded array.
///
/// # Safety
/// `ptr` must address `len` initialized contiguous bytes and be the unique
/// alias whenever the probe admits reads or writes.
pub unsafe fn borrowed_mut_u8(
    ptr: *mut u8,
    len: usize,
    shape: &[usize],
    probe: Arc<dyn BorrowProbe>,
) -> PyResult<PyArray> {
    // SAFETY: forwarded to the caller's contract above.
    let storage = unsafe { ArrayStorage::borrowed_mut_u8(ptr, len, probe) };
    let core = DenseArrayCore::from_storage(storage, shape).map_err(kernels::map_array_err)?;
    Ok(PyArray { core })
}

/// Wrap owned `uint8` data as a writable bounded array.
pub fn owned_u8(data: Vec<u8>, shape: &[usize]) -> PyResult<PyArray> {
    let core = DenseArrayCore::from_storage(ArrayStorage::Uint8(data), shape)
        .map_err(kernels::map_array_err)?;
    Ok(PyArray { core })
}

/// Build the bounded array module: classes, functions, dtype constants, and
/// implementation metadata.
pub fn build_module<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    let m = PyModule::new(py, name)?;
    m.add_class::<PyArray>()?;
    m.add_class::<ArrayLens>()?;
    m.add_class::<PyDType>()?;
    funcs::register(&m)?;

    for dt in [
        ArrayDType::Float32,
        ArrayDType::Float64,
        ArrayDType::Int64,
        ArrayDType::Int32,
        ArrayDType::Uint32,
        ArrayDType::Uint16,
        ArrayDType::Uint8,
        ArrayDType::Bool,
    ] {
        // `bool_` is the Python constant spelling; `dtype` names it `bool`.
        let attr = if dt == ArrayDType::Bool {
            "bool_"
        } else {
            dt.name()
        };
        m.add(attr, PyDType::new(dt))?;
    }

    m.add("__pybevy_implementation__", "pybevy")?;
    Ok(m)
}

/// Register the bounded module as `_pybevy.array`.
pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = build_module(parent.py(), "array")?;
    parent.add_submodule(&m)
}
