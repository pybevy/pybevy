//! CPython DLPack import and export for bounded CPU arrays.
//!
//! A zero-copy export holds the backing's exclusive operation guard in
//! `manager_ctx`. This is intentionally stronger than a read guard: DLPack's
//! read-only flag is advisory, so a consumer may still write through the
//! exported pointer. The exclusive guard makes all concurrent PyBevy access
//! fail cleanly until the consumer invokes the managed-tensor deleter.

use std::{
    ffi::{CStr, c_void},
    mem::align_of,
    ptr::{NonNull, null_mut},
    slice,
};

use pyo3::{
    exceptions::{PyBufferError, PyTypeError, PyValueError},
    ffi,
    prelude::*,
    types::{PyCapsule, PyDict},
};

use super::{array::PyArray, kernels::map_array_err};
use crate::{
    ArrayDType, ArrayStorage, DenseArrayCore, MAX_NDIM, Scalar,
    backing::ArrayWriteGuard,
    shape::{c_contiguous_strides, checked_num_elements},
};

const CPU_DEVICE_TYPE: i32 = 1;
const CPU_DEVICE_ID: i32 = 0;
const DLPACK_MAJOR_VERSION: u32 = 1;
const DLPACK_MINOR_VERSION: u32 = 3;
const DLPACK_FLAG_READ_ONLY: u64 = 1;
const DLPACK_FLAG_BITMASK_IS_COPIED: u64 = 2;
const DLPACK_FLAG_BITMASK_IS_SUBBYTE_TYPE_PADDED: u64 = 4;
const DLPACK_KNOWN_FLAGS: u64 = DLPACK_FLAG_READ_ONLY
    | DLPACK_FLAG_BITMASK_IS_COPIED
    | DLPACK_FLAG_BITMASK_IS_SUBBYTE_TYPE_PADDED;

const LEGACY_CAPSULE_NAME: &CStr = c"dltensor";
const USED_LEGACY_CAPSULE_NAME: &CStr = c"used_dltensor";
const VERSIONED_CAPSULE_NAME: &CStr = c"dltensor_versioned";
const USED_VERSIONED_CAPSULE_NAME: &CStr = c"used_dltensor_versioned";

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct DlDevice {
    device_type: i32,
    device_id: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct DlDataType {
    code: u8,
    bits: u8,
    lanes: u16,
}

#[repr(C)]
#[derive(Debug)]
struct DlTensor {
    data: *mut c_void,
    device: DlDevice,
    ndim: i32,
    dtype: DlDataType,
    shape: *mut i64,
    strides: *mut i64,
    byte_offset: u64,
}

#[repr(C)]
struct DlManagedTensor {
    dl_tensor: DlTensor,
    manager_ctx: *mut c_void,
    deleter: Option<unsafe extern "C" fn(*mut DlManagedTensor)>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct DlPackVersion {
    major: u32,
    minor: u32,
}

#[repr(C)]
struct DlManagedTensorVersioned {
    version: DlPackVersion,
    manager_ctx: *mut c_void,
    deleter: Option<unsafe extern "C" fn(*mut DlManagedTensorVersioned)>,
    flags: u64,
    dl_tensor: DlTensor,
}

enum ExportOwner {
    Guard { _guard: ArrayWriteGuard },
    BoolBytes { _bytes: Box<[u8]> },
}

struct DlpackManager {
    _owner: ExportOwner,
    shape: Box<[i64]>,
    strides: Box<[i64]>,
}

struct PreparedExport {
    manager: Box<DlpackManager>,
    data: *mut c_void,
    ndim: i32,
    dtype: DlDataType,
    flags: u64,
}

fn pointer_or_null(values: &mut [i64]) -> *mut i64 {
    if values.is_empty() {
        null_mut()
    } else {
        values.as_mut_ptr()
    }
}

fn explicit_strides(shape: &[i64]) -> PyResult<Box<[i64]>> {
    let mut strides = vec![0_i64; shape.len()];
    let mut stride = 1_i64;
    for (index, dimension) in shape.iter().enumerate().rev() {
        strides[index] = stride;
        stride = stride
            .checked_mul(*dimension)
            .ok_or_else(|| PyBufferError::new_err("array strides exceed DLPack i64"))?;
    }
    Ok(strides.into_boxed_slice())
}

fn dl_dtype(dtype: ArrayDType) -> DlDataType {
    let dtype = dtype.dlpack();
    DlDataType {
        code: dtype.code,
        bits: dtype.bits,
        lanes: dtype.lanes,
    }
}

fn owned_guard(core: DenseArrayCore) -> PyResult<ArrayWriteGuard> {
    let guard = core.exclusive_storage().map_err(map_array_err)?;
    drop(core);
    Ok(guard)
}

fn copied_owner(core: &DenseArrayCore) -> PyResult<(ExportOwner, *mut c_void)> {
    if core.dtype() == ArrayDType::Bool {
        let mut bytes = core
            .to_scalars()
            .map_err(map_array_err)?
            .into_iter()
            .map(|value| u8::from(value.to_bool()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let data = if bytes.is_empty() {
            null_mut()
        } else {
            bytes.as_mut_ptr().cast::<c_void>()
        };
        return Ok((ExportOwner::BoolBytes { _bytes: bytes }, data));
    }

    let copied = core.copy().map_err(map_array_err)?;
    let mut guard = owned_guard(copied)?;
    let data = if guard.is_empty() {
        null_mut()
    } else {
        guard
            .as_mut_contiguous_ptr()
            .expect("an owned non-boolean array has byte-addressable storage")
            .cast::<c_void>()
    };
    Ok((ExportOwner::Guard { _guard: guard }, data))
}

fn prepare_export(core: &DenseArrayCore, copy: Option<bool>) -> PyResult<PreparedExport> {
    let elements = checked_num_elements(core.shape()).map_err(map_array_err)?;
    let expected_bytes = elements
        .checked_mul(core.dtype().itemsize())
        .ok_or_else(|| PyBufferError::new_err("array byte length overflows usize"))?;

    let needs_copy = {
        let storage = core.read_storage().map_err(map_array_err)?;
        core.dtype() == ArrayDType::Bool
            || core.is_view()
            || storage.is_borrowed()
            || !core.is_c_contiguous()
            || core.layout().offset != 0
            || elements != storage.len()
    };

    if copy == Some(false) && needs_copy {
        return Err(PyBufferError::new_err(
            "this array requires a copy for DLPack export",
        ));
    }

    let copied = copy == Some(true) || needs_copy;
    let (owner, data) = if copied {
        copied_owner(core)?
    } else {
        let mut guard = core.exclusive_storage().map_err(map_array_err)?;
        let actual_bytes = guard
            .len()
            .checked_mul(core.dtype().itemsize())
            .ok_or_else(|| PyBufferError::new_err("array byte length overflows usize"))?;
        if actual_bytes != expected_bytes {
            return Err(PyBufferError::new_err(
                "array storage length changed during DLPack export",
            ));
        }
        let data = if guard.is_empty() {
            null_mut()
        } else {
            guard
                .as_mut_contiguous_ptr()
                .ok_or_else(|| PyBufferError::new_err("array storage is not exportable"))?
                .cast::<c_void>()
        };
        (ExportOwner::Guard { _guard: guard }, data)
    };

    let shape = core
        .shape()
        .iter()
        .map(|dimension| {
            i64::try_from(*dimension)
                .map_err(|_| PyBufferError::new_err("array dimension exceeds DLPack i64"))
        })
        .collect::<PyResult<Box<[_]>>>()?;
    let strides = explicit_strides(&shape)?;
    let ndim = i32::try_from(shape.len())
        .map_err(|_| PyBufferError::new_err("array rank exceeds DLPack i32"))?;
    let flags = if copied {
        DLPACK_FLAG_BITMASK_IS_COPIED
    } else if core.is_writable() {
        0
    } else {
        DLPACK_FLAG_READ_ONLY
    };

    Ok(PreparedExport {
        manager: Box::new(DlpackManager {
            _owner: owner,
            shape,
            strides,
        }),
        data,
        ndim,
        dtype: dl_dtype(core.dtype()),
        flags,
    })
}

fn dl_tensor(prepared: &mut PreparedExport) -> DlTensor {
    DlTensor {
        data: prepared.data,
        device: DlDevice {
            device_type: CPU_DEVICE_TYPE,
            device_id: CPU_DEVICE_ID,
        },
        ndim: prepared.ndim,
        dtype: prepared.dtype,
        shape: pointer_or_null(&mut prepared.manager.shape),
        strides: pointer_or_null(&mut prepared.manager.strides),
        byte_offset: 0,
    }
}

unsafe extern "C" fn delete_legacy_managed(tensor: *mut DlManagedTensor) {
    if tensor.is_null() {
        return;
    }
    // SAFETY: the pointer and manager were allocated together by
    // `legacy_capsule` and ownership reaches this deleter exactly once.
    let tensor = unsafe { Box::from_raw(tensor) };
    if !tensor.manager_ctx.is_null() {
        // SAFETY: `manager_ctx` is the Box<DlpackManager> paired with `tensor`.
        drop(unsafe { Box::from_raw(tensor.manager_ctx.cast::<DlpackManager>()) });
    }
}

unsafe extern "C" fn delete_versioned_managed(tensor: *mut DlManagedTensorVersioned) {
    if tensor.is_null() {
        return;
    }
    // SAFETY: the pointer and manager were allocated together by
    // `versioned_capsule` and ownership reaches this deleter exactly once.
    let tensor = unsafe { Box::from_raw(tensor) };
    if !tensor.manager_ctx.is_null() {
        // SAFETY: `manager_ctx` is the Box<DlpackManager> paired with `tensor`.
        drop(unsafe { Box::from_raw(tensor.manager_ctx.cast::<DlpackManager>()) });
    }
}

unsafe extern "C" fn delete_legacy_capsule(capsule: *mut ffi::PyObject) {
    // SAFETY: CPython supplies a live capsule. A consumer renames an adopted
    // capsule, in which case it owns and eventually deletes the tensor.
    if unsafe { ffi::PyCapsule_IsValid(capsule, LEGACY_CAPSULE_NAME.as_ptr()) } == 0 {
        return;
    }
    // SAFETY: validity above proves the pointer has the expected managed type.
    let tensor = unsafe { ffi::PyCapsule_GetPointer(capsule, LEGACY_CAPSULE_NAME.as_ptr()) }
        .cast::<DlManagedTensor>();
    if !tensor.is_null() {
        // SAFETY: the original capsule name proves ownership was not adopted.
        unsafe { delete_legacy_managed(tensor) };
    }
}

unsafe extern "C" fn delete_versioned_capsule(capsule: *mut ffi::PyObject) {
    // SAFETY: CPython supplies a live capsule; renamed capsules are consumer-owned.
    if unsafe { ffi::PyCapsule_IsValid(capsule, VERSIONED_CAPSULE_NAME.as_ptr()) } == 0 {
        return;
    }
    // SAFETY: validity above proves the pointer has the expected managed type.
    let tensor = unsafe { ffi::PyCapsule_GetPointer(capsule, VERSIONED_CAPSULE_NAME.as_ptr()) }
        .cast::<DlManagedTensorVersioned>();
    if !tensor.is_null() {
        // SAFETY: the original capsule name proves ownership was not adopted.
        unsafe { delete_versioned_managed(tensor) };
    }
}

fn legacy_capsule(py: Python<'_>, mut prepared: PreparedExport) -> PyResult<Bound<'_, PyCapsule>> {
    let tensor = dl_tensor(&mut prepared);
    let manager = Box::into_raw(prepared.manager).cast::<c_void>();
    let managed = Box::new(DlManagedTensor {
        dl_tensor: tensor,
        manager_ctx: manager,
        deleter: Some(delete_legacy_managed),
    });
    let pointer = NonNull::new(Box::into_raw(managed).cast::<c_void>())
        .expect("Box never yields a null DLPack pointer");
    // SAFETY: `pointer` owns a valid managed tensor and the capsule destructor
    // releases it only if no consumer adopts it.
    let capsule = unsafe {
        PyCapsule::new_with_pointer_and_destructor(
            py,
            pointer,
            LEGACY_CAPSULE_NAME,
            Some(delete_legacy_capsule),
        )
    };
    if capsule.is_err() {
        // SAFETY: capsule construction failed, so ownership never escaped.
        unsafe { delete_legacy_managed(pointer.as_ptr().cast::<DlManagedTensor>()) };
    }
    capsule
}

fn versioned_capsule(
    py: Python<'_>,
    mut prepared: PreparedExport,
) -> PyResult<Bound<'_, PyCapsule>> {
    let tensor = dl_tensor(&mut prepared);
    let flags = prepared.flags;
    let manager = Box::into_raw(prepared.manager).cast::<c_void>();
    let managed = Box::new(DlManagedTensorVersioned {
        version: DlPackVersion {
            major: DLPACK_MAJOR_VERSION,
            minor: DLPACK_MINOR_VERSION,
        },
        manager_ctx: manager,
        deleter: Some(delete_versioned_managed),
        flags,
        dl_tensor: tensor,
    });
    let pointer = NonNull::new(Box::into_raw(managed).cast::<c_void>())
        .expect("Box never yields a null DLPack pointer");
    // SAFETY: `pointer` owns a valid versioned managed tensor and the capsule
    // destructor releases it only if no consumer adopts it.
    let capsule = unsafe {
        PyCapsule::new_with_pointer_and_destructor(
            py,
            pointer,
            VERSIONED_CAPSULE_NAME,
            Some(delete_versioned_capsule),
        )
    };
    if capsule.is_err() {
        // SAFETY: capsule construction failed, so ownership never escaped.
        unsafe {
            delete_versioned_managed(pointer.as_ptr().cast::<DlManagedTensorVersioned>());
        }
    }
    capsule
}

pub(super) fn export<'py>(
    core: &DenseArrayCore,
    py: Python<'py>,
    stream: Option<&Bound<'_, PyAny>>,
    max_version: Option<(u32, u32)>,
    dl_device: Option<(i32, i32)>,
    copy: Option<bool>,
) -> PyResult<Bound<'py, PyCapsule>> {
    if stream.is_some_and(|stream| !stream.is_none()) {
        return Err(PyValueError::new_err(
            "CPU DLPack export requires stream=None",
        ));
    }
    if let Some(device) = dl_device
        && device != (CPU_DEVICE_TYPE, CPU_DEVICE_ID)
    {
        return Err(PyBufferError::new_err(format!(
            "Array is on CPU device (1, 0), not requested device {device:?}"
        )));
    }
    let prepared = prepare_export(core, copy)?;
    match max_version {
        Some((major, _)) if major >= DLPACK_MAJOR_VERSION => versioned_capsule(py, prepared),
        _ => legacy_capsule(py, prepared),
    }
}

fn dtype_from_dlpack(dtype: DlDataType) -> PyResult<ArrayDType> {
    if dtype.lanes != 1 {
        return Err(PyBufferError::new_err(
            "DLPack tensors with lanes != 1 are unsupported",
        ));
    }
    match (dtype.code, dtype.bits) {
        (2, 16) => Ok(ArrayDType::Float16),
        (2, 32) => Ok(ArrayDType::Float32),
        (2, 64) => Ok(ArrayDType::Float64),
        (0, 64) => Ok(ArrayDType::Int64),
        (0, 32) => Ok(ArrayDType::Int32),
        (1, 32) => Ok(ArrayDType::Uint32),
        (1, 16) => Ok(ArrayDType::Uint16),
        (1, 8) => Ok(ArrayDType::Uint8),
        (6, 8) => Ok(ArrayDType::Bool),
        _ => Err(PyBufferError::new_err(format!(
            "unsupported DLPack dtype (code={}, bits={}, lanes={})",
            dtype.code, dtype.bits, dtype.lanes
        ))),
    }
}

fn dtype_alignment(dtype: ArrayDType) -> usize {
    match dtype {
        ArrayDType::Float16 => align_of::<u16>(),
        ArrayDType::Float32 => align_of::<f32>(),
        ArrayDType::Float64 => align_of::<f64>(),
        ArrayDType::Int64 => align_of::<i64>(),
        ArrayDType::Int32 => align_of::<i32>(),
        ArrayDType::Uint32 => align_of::<u32>(),
        ArrayDType::Uint16 => align_of::<u16>(),
        ArrayDType::Uint8 | ArrayDType::Bool => align_of::<u8>(),
    }
}

unsafe fn copy_tensor(tensor: &DlTensor) -> PyResult<DenseArrayCore> {
    if tensor.device.device_type != CPU_DEVICE_TYPE || tensor.device.device_id != CPU_DEVICE_ID {
        return Err(PyBufferError::new_err(format!(
            "from_dlpack requires CPU device (1, 0), got ({}, {})",
            tensor.device.device_type, tensor.device.device_id
        )));
    }
    let ndim = usize::try_from(tensor.ndim)
        .map_err(|_| PyBufferError::new_err("DLPack tensor rank must be non-negative"))?;
    if ndim > MAX_NDIM {
        return Err(PyBufferError::new_err(format!(
            "DLPack tensor rank {ndim} exceeds maximum {MAX_NDIM}"
        )));
    }
    if ndim > 0 && tensor.shape.is_null() {
        return Err(PyBufferError::new_err(
            "DLPack tensor has a null shape pointer",
        ));
    }
    let raw_shape = if ndim == 0 {
        &[][..]
    } else {
        // SAFETY: the producer contract keeps `ndim` shape entries live until
        // its deleter runs; rank was bounded above before this read.
        unsafe { slice::from_raw_parts(tensor.shape.cast_const(), ndim) }
    };
    let shape = raw_shape
        .iter()
        .map(|dimension| {
            usize::try_from(*dimension)
                .map_err(|_| PyBufferError::new_err("DLPack dimensions must be non-negative"))
        })
        .collect::<PyResult<Vec<_>>>()?;
    let elements = checked_num_elements(&shape).map_err(map_array_err)?;

    let expected_strides = c_contiguous_strides(&shape);
    if !tensor.strides.is_null() {
        // SAFETY: the producer contract keeps `ndim` stride entries live until
        // its deleter runs; rank was bounded above before this read.
        let strides = unsafe { slice::from_raw_parts(tensor.strides.cast_const(), ndim) };
        for (actual, expected) in strides.iter().zip(expected_strides.iter()) {
            let expected = i64::try_from(*expected)
                .map_err(|_| PyBufferError::new_err("DLPack stride exceeds i64"))?;
            if *actual != expected {
                return Err(PyBufferError::new_err(
                    "from_dlpack requires a C-contiguous tensor",
                ));
            }
        }
    }

    let dtype = dtype_from_dlpack(tensor.dtype)?;
    let byte_len = elements
        .checked_mul(dtype.itemsize())
        .ok_or_else(|| PyBufferError::new_err("DLPack tensor byte length overflows usize"))?;
    let byte_offset = usize::try_from(tensor.byte_offset)
        .map_err(|_| PyBufferError::new_err("DLPack byte offset exceeds pointer width"))?;
    let address = (tensor.data as usize)
        .checked_add(byte_offset)
        .ok_or_else(|| PyBufferError::new_err("DLPack data address overflows pointer width"))?;
    if byte_len > 0 && tensor.data.is_null() {
        return Err(PyBufferError::new_err(
            "non-empty DLPack tensor has a null data pointer",
        ));
    }
    if address % dtype_alignment(dtype) != 0 {
        return Err(PyBufferError::new_err(
            "DLPack data pointer is not aligned for its dtype",
        ));
    }
    let bytes = if byte_len == 0 {
        &[][..]
    } else {
        // SAFETY: the validated tensor metadata describes `byte_len` readable
        // CPU bytes, and the producer retains them until its deleter runs.
        unsafe { slice::from_raw_parts(address as *const u8, byte_len) }
    };
    let mut storage = ArrayStorage::zeros(dtype, elements).map_err(map_array_err)?;
    if dtype == ArrayDType::Float16 {
        for index in 0..elements {
            let start = index * 2;
            storage.set_float16_bits(
                index,
                u16::from_ne_bytes(
                    bytes[start..start + 2]
                        .try_into()
                        .expect("validated float16 item width"),
                ),
            );
        }
        return DenseArrayCore::from_storage(storage, &shape).map_err(map_array_err);
    }
    for index in 0..elements {
        let start = index * dtype.itemsize();
        let scalar = match dtype {
            ArrayDType::Float16 => unreachable!("float16 copied as raw bits above"),
            ArrayDType::Float32 => Scalar::F64(f32::from_ne_bytes(
                bytes[start..start + 4]
                    .try_into()
                    .expect("validated f32 item width"),
            ) as f64),
            ArrayDType::Float64 => Scalar::F64(f64::from_ne_bytes(
                bytes[start..start + 8]
                    .try_into()
                    .expect("validated f64 item width"),
            )),
            ArrayDType::Int64 => Scalar::I64(i64::from_ne_bytes(
                bytes[start..start + 8]
                    .try_into()
                    .expect("validated i64 item width"),
            )),
            ArrayDType::Int32 => Scalar::I64(i32::from_ne_bytes(
                bytes[start..start + 4]
                    .try_into()
                    .expect("validated i32 item width"),
            ) as i64),
            ArrayDType::Uint32 => Scalar::I64(u32::from_ne_bytes(
                bytes[start..start + 4]
                    .try_into()
                    .expect("validated u32 item width"),
            ) as i64),
            ArrayDType::Uint16 => Scalar::I64(u16::from_ne_bytes(
                bytes[start..start + 2]
                    .try_into()
                    .expect("validated u16 item width"),
            ) as i64),
            ArrayDType::Uint8 => Scalar::I64(i64::from(bytes[start])),
            ArrayDType::Bool => {
                if bytes[start] > 1 {
                    return Err(PyBufferError::new_err(
                        "DLPack bool tensor contains a value other than 0 or 1",
                    ));
                }
                Scalar::Bool(bytes[start] != 0)
            }
        };
        storage.set(index, scalar);
    }
    DenseArrayCore::from_storage(storage, &shape).map_err(map_array_err)
}

unsafe fn consume_legacy(
    py: Python<'_>,
    tensor: *mut DlManagedTensor,
    capsule: *mut ffi::PyObject,
) -> PyResult<()> {
    // SAFETY: caller validated the capsule name and non-null managed pointer.
    let deleter = unsafe { (*tensor).deleter }
        .ok_or_else(|| PyBufferError::new_err("DLPack managed tensor has no deleter"))?;
    // SAFETY: the static replacement name outlives the capsule.
    if unsafe { ffi::PyCapsule_SetName(capsule, USED_LEGACY_CAPSULE_NAME.as_ptr()) } != 0 {
        return Err(PyErr::fetch(py));
    }
    // SAFETY: renaming transfers ownership to this importer, which invokes the
    // producer's deleter exactly once after finishing its copy.
    unsafe { deleter(tensor) };
    Ok(())
}

unsafe fn consume_versioned(
    py: Python<'_>,
    tensor: *mut DlManagedTensorVersioned,
    capsule: *mut ffi::PyObject,
) -> PyResult<()> {
    // SAFETY: caller validated the capsule name and non-null managed pointer.
    let deleter = unsafe { (*tensor).deleter }
        .ok_or_else(|| PyBufferError::new_err("DLPack managed tensor has no deleter"))?;
    // SAFETY: the static replacement name outlives the capsule.
    if unsafe { ffi::PyCapsule_SetName(capsule, USED_VERSIONED_CAPSULE_NAME.as_ptr()) } != 0 {
        return Err(PyErr::fetch(py));
    }
    // SAFETY: renaming transfers ownership to this importer, which invokes the
    // producer's deleter exactly once after finishing its copy.
    unsafe { deleter(tensor) };
    Ok(())
}

pub(super) fn from_dlpack(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<PyArray> {
    let device: (i32, i32) = obj.call_method0("__dlpack_device__")?.extract()?;
    if device != (CPU_DEVICE_TYPE, CPU_DEVICE_ID) {
        return Err(PyBufferError::new_err(format!(
            "from_dlpack requires CPU device (1, 0), got {device:?}"
        )));
    }

    let kwargs = PyDict::new(py);
    kwargs.set_item("max_version", (DLPACK_MAJOR_VERSION, DLPACK_MINOR_VERSION))?;
    let capsule = match obj.call_method("__dlpack__", (), Some(&kwargs)) {
        Ok(capsule) => capsule,
        Err(error) if error.is_instance_of::<PyTypeError>(py) => obj.call_method0("__dlpack__")?,
        Err(error) => return Err(error),
    };
    let capsule_ptr = capsule.as_ptr();

    // SAFETY: the validity checks establish both the capsule kind and pointer
    // type. The producer contract keeps metadata/data live through the copy.
    unsafe {
        if ffi::PyCapsule_IsValid(capsule_ptr, VERSIONED_CAPSULE_NAME.as_ptr()) != 0 {
            let managed = ffi::PyCapsule_GetPointer(capsule_ptr, VERSIONED_CAPSULE_NAME.as_ptr())
                .cast::<DlManagedTensorVersioned>();
            if managed.is_null() {
                return Err(PyBufferError::new_err("DLPack capsule has a null pointer"));
            }
            let version = (*managed).version;
            if version.major != DLPACK_MAJOR_VERSION || version.minor > DLPACK_MINOR_VERSION {
                return Err(PyBufferError::new_err(format!(
                    "unsupported DLPack version {}.{}",
                    version.major, version.minor
                )));
            }
            if (*managed).flags & !DLPACK_KNOWN_FLAGS != 0 {
                return Err(PyBufferError::new_err("DLPack tensor has unknown flags"));
            }
            if (*managed).deleter.is_none() {
                return Err(PyBufferError::new_err(
                    "DLPack managed tensor has no deleter",
                ));
            }
            let core = copy_tensor(&(*managed).dl_tensor)?;
            consume_versioned(py, managed, capsule_ptr)?;
            return Ok(PyArray::wrap(core));
        }
        if ffi::PyCapsule_IsValid(capsule_ptr, LEGACY_CAPSULE_NAME.as_ptr()) != 0 {
            let managed = ffi::PyCapsule_GetPointer(capsule_ptr, LEGACY_CAPSULE_NAME.as_ptr())
                .cast::<DlManagedTensor>();
            if managed.is_null() {
                return Err(PyBufferError::new_err("DLPack capsule has a null pointer"));
            }
            if (*managed).deleter.is_none() {
                return Err(PyBufferError::new_err(
                    "DLPack managed tensor has no deleter",
                ));
            }
            let core = copy_tensor(&(*managed).dl_tensor)?;
            consume_legacy(py, managed, capsule_ptr)?;
            return Ok(PyArray::wrap(core));
        }
    }

    Err(PyBufferError::new_err(
        "__dlpack__ returned an invalid or already-consumed capsule",
    ))
}
