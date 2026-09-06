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
mod dlpack;
mod dtype;
mod funcs;
mod hint;
mod kernels;
mod lens;

use std::sync::Arc;

pub use array::PyArray;
pub use dtype::PyDType;
pub use lens::ArrayLens;
use pyo3::{buffer::PyUntypedBuffer, exceptions::PyTypeError, prelude::*, types::PyModule};

use crate::{
    ArrayDType, ArrayStorage, BorrowProbe, DenseArrayCore, decode_read_only_le_bytes,
    encode_contiguous_le_bytes,
};

type EncodedArray = (ArrayDType, Vec<usize>, Vec<u8>);

/// Wrap owned `float32` data as a read-only bounded array of `shape`. Engine
/// APIs (mesh, image) use this to hand back attribute snapshots that carry the
/// bounded-array surface. Writing to the result raises because it is a copy.
pub fn read_only_f32(data: Vec<f32>, shape: &[usize]) -> PyResult<PyArray> {
    let core = crate::kernels::read_only_f32_core(data, shape).map_err(kernels::map_kernel_err)?;
    Ok(PyArray::wrap(core))
}

/// Decode shared little-endian tensor storage into a read-only Python array.
pub fn read_only_from_le_bytes(
    dtype: ArrayDType,
    bytes: &[u8],
    shape: &[usize],
) -> PyResult<PyArray> {
    let core = decode_read_only_le_bytes(dtype, bytes, shape).map_err(kernels::map_array_err)?;
    Ok(PyArray::wrap(core))
}

/// Copy a Python array's logical C order into shared little-endian storage.
pub fn owned_contiguous_le_bytes(array: &PyArray) -> PyResult<(ArrayDType, Vec<usize>, Vec<u8>)> {
    encode_contiguous_le_bytes(&array.core).map_err(kernels::map_array_err)
}

/// Normalize a bounded array, Python scalar/sequence, or loaded NumPy value
/// into owned contiguous little-endian storage.
///
/// Native values stay on the portable bounded-array path. NumPy inspection is
/// confined to this PyO3 adapter and always snapshots external storage first.
pub fn owned_contiguous_le_bytes_from_any(
    value: &Bound<'_, PyAny>,
    dtype: Option<ArrayDType>,
) -> PyResult<(ArrayDType, Vec<usize>, Vec<u8>)> {
    if let Some(encoded) = snapshot_compatible_buffer(value, dtype)? {
        return Ok(encoded);
    }
    let core = convert::array_from_object(value, dtype)?;
    crate::encode_contiguous_le_bytes(&core).map_err(kernels::map_array_err)
}

fn buffer_dtype(buffer: &PyUntypedBuffer) -> Option<ArrayDType> {
    if !cfg!(target_endian = "little") {
        return None;
    }
    let format = buffer.format().to_bytes();
    let code = match format {
        [code] | [b'@' | b'=' | b'<', code] => *code,
        _ => return None,
    };
    let dtype = match (code, buffer.item_size()) {
        (b'e', 2) => ArrayDType::Float16,
        (b'f', 4) => ArrayDType::Float32,
        (b'd', 8) => ArrayDType::Float64,
        (b'q' | b'l', 8) => ArrayDType::Int64,
        (b'i' | b'l', 4) => ArrayDType::Int32,
        (b'I' | b'L', 4) => ArrayDType::Uint32,
        (b'H', 2) => ArrayDType::Uint16,
        (b'B', 1) => ArrayDType::Uint8,
        (b'?', 1) => ArrayDType::Bool,
        _ => return None,
    };
    Some(dtype)
}

fn copy_buffer_bytes(buffer: &PyUntypedBuffer) -> Vec<u8> {
    let len = buffer.len_bytes();
    if len == 0 {
        return Vec::new();
    }
    // SAFETY: the caller holds the exporting buffer for this synchronous copy,
    // validated it as C-contiguous, and does not re-enter Python while reading.
    unsafe { std::slice::from_raw_parts(buffer.buf_ptr().cast::<u8>(), len) }.to_vec()
}

fn encode_compatible_buffer(
    value: &Bound<'_, PyAny>,
    dtype: Option<ArrayDType>,
    private_snapshot: bool,
) -> PyResult<Option<EncodedArray>> {
    let Ok(buffer) = PyUntypedBuffer::get(value) else {
        return Ok(None);
    };
    let Some(buffer_dtype) = buffer_dtype(&buffer) else {
        return Ok(None);
    };
    if dtype.is_some_and(|dtype| dtype != buffer_dtype)
        || !buffer.is_c_contiguous()
        || (!private_snapshot && !buffer.readonly())
    {
        return Ok(None);
    }
    let shape = buffer.shape().to_vec();
    if crate::checked_num_elements(&shape).ok() != Some(buffer.item_count()) {
        return Ok(None);
    }
    let bytes = copy_buffer_bytes(&buffer);
    Ok(Some((buffer_dtype, shape, bytes)))
}

fn snapshot_compatible_buffer(
    value: &Bound<'_, PyAny>,
    dtype: Option<ArrayDType>,
) -> PyResult<Option<EncodedArray>> {
    if convert::is_numpy_array(value)? {
        let Ok(buffer) = PyUntypedBuffer::get(value) else {
            return Ok(None);
        };
        let Some(buffer_dtype) = buffer_dtype(&buffer) else {
            return Ok(None);
        };
        if dtype.is_some_and(|dtype| dtype != buffer_dtype) || !buffer.is_c_contiguous() {
            return Ok(None);
        }
        let snapshot = convert::numpy_contiguous_snapshot(value)?;
        return encode_compatible_buffer(&snapshot, dtype, true);
    }
    encode_compatible_buffer(value, dtype, false)
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
    Ok(PyArray::wrap(core))
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
    Ok(PyArray::wrap(core))
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
    Ok(PyArray::wrap(core))
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
    Ok(PyArray::wrap(core))
}

/// Wrap owned `uint8` data as a writable bounded array.
pub fn owned_u8(data: Vec<u8>, shape: &[usize]) -> PyResult<PyArray> {
    let core = DenseArrayCore::from_storage(ArrayStorage::Uint8(data), shape)
        .map_err(kernels::map_array_err)?;
    Ok(PyArray::wrap(core))
}

/// Copy a bounded or real-NumPy array into owned `uint8` data and preserve its
/// logical shape. Returns `None` when `obj` is neither supported array type.
///
/// NumPy inputs go through the adapter's private base-ndarray snapshot before
/// Rust reads them, retaining the free-threading safety contract of
/// `array()`/`asarray()`.
pub fn extract_u8_array_data(obj: &Bound<'_, PyAny>) -> PyResult<Option<(Vec<u8>, Vec<usize>)>> {
    let is_bounded = obj.extract::<PyRef<'_, PyArray>>().is_ok();
    let is_numpy = convert::is_numpy_array(obj)?;
    if !is_bounded && !is_numpy {
        return Ok(None);
    }

    let core = convert::array_from_object(obj, None)?;
    if core.dtype() != ArrayDType::Uint8 {
        return Err(PyTypeError::new_err(format!(
            "array data must have dtype uint8, got {}",
            core.dtype().name()
        )));
    }
    let shape = core.shape().to_vec();
    let data = core
        .to_scalars()
        .map_err(kernels::map_array_err)?
        .into_iter()
        .map(|value| value.to_i64_trunc() as u8)
        .collect();
    Ok(Some((data, shape)))
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
        ArrayDType::Float16,
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
