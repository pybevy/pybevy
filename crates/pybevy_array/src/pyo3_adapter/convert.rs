//! Python <-> bounded array conversion: nested-list construction, `tolist`,
//! shape parsing, and copy-based real-NumPy interop.

use numpy::{PyArray1, PyArrayMethods, PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyModule, PySequence, PyString, PyTuple},
};

use super::{
    array::PyArray,
    kernels::{extract_scalar, map_array_err, scalar_to_py},
};
use crate::{
    ArrayDType, ArrayStorage, DenseArrayCore, Layout, MAX_NDIM, Scalar, checked_num_elements,
};

fn is_sequence(obj: &Bound<'_, PyAny>) -> bool {
    if obj.is_instance_of::<PyString>() {
        return false;
    }
    obj.is_instance_of::<PyList>() || obj.is_instance_of::<PyTuple>()
}

fn is_python_scalar(obj: &Bound<'_, PyAny>) -> bool {
    obj.is_instance_of::<PyBool>()
        || obj.is_instance_of::<PyInt>()
        || obj.is_instance_of::<PyFloat>()
}

fn loaded_numpy<'py>(py: Python<'py>) -> PyResult<Option<Bound<'py, PyModule>>> {
    let modules = PyModule::import(py, "sys")?
        .getattr("modules")?
        .cast_into::<PyDict>()?;
    Ok(modules
        .get_item("numpy")?
        .map(|module| module.cast_into::<PyModule>())
        .transpose()?)
}

pub(super) fn is_numpy_array(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Some(numpy) = loaded_numpy(obj.py())? else {
        return Ok(false);
    };
    obj.is_instance(&numpy.getattr("ndarray")?)
}

/// Recursively determine shape and flattened values, rejecting ragged nesting.
fn collect(obj: &Bound<'_, PyAny>, depth: usize) -> PyResult<(Vec<usize>, Vec<Scalar>)> {
    if !is_sequence(obj) {
        return Ok((Vec::new(), vec![extract_scalar(obj)?]));
    }
    if depth == MAX_NDIM {
        return Err(PyValueError::new_err(format!(
            "maximum supported array dimensionality is {MAX_NDIM}"
        )));
    }
    let seq = obj.cast::<PySequence>()?;
    let len = seq.len()?;
    if len == 0 {
        return Ok((vec![0], Vec::new()));
    }
    let mut subshape: Option<Vec<usize>> = None;
    let mut flat: Vec<Scalar> = Vec::new();
    for i in 0..len {
        let item = seq.get_item(i)?;
        let (sh, fl) = collect(&item, depth + 1)?;
        match &subshape {
            None => subshape = Some(sh),
            Some(expected) if *expected != sh => {
                return Err(PyValueError::new_err(
                    "setting an array element with a ragged nested sequence",
                ));
            }
            _ => {}
        }
        flat.extend(fl);
    }
    let mut shape = vec![len];
    shape.extend(subshape.unwrap_or_default());
    Ok((shape, flat))
}

fn infer_dtype(flat: &[Scalar]) -> ArrayDType {
    let mut has_float = false;
    let mut has_int = false;
    let mut has_bool = false;
    for s in flat {
        match s {
            Scalar::F64(_) => has_float = true,
            Scalar::I64(_) => has_int = true,
            Scalar::Bool(_) => has_bool = true,
        }
    }
    if has_float {
        ArrayDType::Float64
    } else if has_int {
        ArrayDType::Int64
    } else if has_bool {
        ArrayDType::Bool
    } else {
        ArrayDType::Float64 // empty -> float64, matching NumPy
    }
}

fn storage_from_scalars(flat: &[Scalar], dtype: ArrayDType) -> PyResult<ArrayStorage> {
    let mut storage = ArrayStorage::zeros(dtype, flat.len()).map_err(map_array_err)?;
    for (i, &s) in flat.iter().enumerate() {
        storage.set(i, s);
    }
    Ok(storage)
}

/// Build a bounded array from a Python scalar or (possibly nested) list/tuple.
pub fn array_from_object(
    obj: &Bound<'_, PyAny>,
    dtype: Option<ArrayDType>,
) -> PyResult<DenseArrayCore> {
    if let Ok(nd) = obj.extract::<PyRef<'_, PyArray>>() {
        let core = nd.core.copy().map_err(map_array_err)?;
        return match dtype {
            Some(dt) if dt != core.dtype() => core.astype(dt).map_err(map_array_err),
            _ => Ok(core),
        };
    }
    // Native Python inputs are the backend-independent path. Handle them
    // before inspecting NumPy so ordinary bounded-array construction neither
    // imports NumPy nor initializes its C API.
    if is_sequence(obj) || is_python_scalar(obj) {
        let (shape, flat) = collect(obj, 0)?;
        let dtype = dtype.unwrap_or_else(|| infer_dtype(&flat));
        let storage = storage_from_scalars(&flat, dtype)?;
        return DenseArrayCore::from_storage(storage, &shape).map_err(map_array_err);
    }
    if is_numpy_array(obj)? {
        let core = array_from_numpy(obj)?;
        return match dtype {
            Some(dt) if dt != core.dtype() => core.astype(dt).map_err(map_array_err),
            _ => Ok(core),
        };
    }
    if let Some(core) = array_from_numpy_scalar(obj)? {
        return match dtype {
            Some(dt) if dt != core.dtype() => core.astype(dt).map_err(map_array_err),
            _ => Ok(core),
        };
    }
    let (shape, flat) = collect(obj, 0)?;
    let dtype = dtype.unwrap_or_else(|| infer_dtype(&flat));
    let storage = storage_from_scalars(&flat, dtype)?;
    DenseArrayCore::from_storage(storage, &shape).map_err(map_array_err)
}

fn dense_core_from_numpy_snapshot<T>(
    array: PyReadonlyArrayDyn<'_, T>,
    storage: impl FnOnce(Vec<T>) -> ArrayStorage,
) -> PyResult<DenseArrayCore>
where
    T: numpy::Element + Copy,
{
    let shape = array.shape().to_vec();
    if shape.len() > MAX_NDIM {
        return Err(PyValueError::new_err(format!(
            "maximum supported array dimensionality is {MAX_NDIM}, got {}",
            shape.len()
        )));
    }
    checked_num_elements(&shape).map_err(map_array_err)?;
    // Iteration follows logical C index order even for strided/transposed
    // NumPy inputs, producing an owned contiguous bounded array.
    let values = array.as_array().iter().copied().collect();
    DenseArrayCore::from_storage(storage(values), &shape).map_err(map_array_err)
}

/// Snapshot a real NumPy ndarray into owned bounded storage.
pub(super) fn numpy_contiguous_snapshot<'py>(
    obj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    // NumPy ndarrays have no object-level mutation lock on free-threaded
    // Python. First ask NumPy to create a private base-ndarray snapshot, so the
    // safe Rust view below cannot alias a Python-visible writable array.
    let kwargs = PyDict::new(obj.py());
    kwargs.set_item("copy", true)?;
    kwargs.set_item("order", "C")?;
    kwargs.set_item("subok", false)?;
    obj.py()
        .import("numpy")?
        .getattr("array")?
        .call((obj,), Some(&kwargs))
}

/// Snapshot a real NumPy ndarray into owned bounded storage.
fn array_from_numpy(obj: &Bound<'_, PyAny>) -> PyResult<DenseArrayCore> {
    let snapshot = numpy_contiguous_snapshot(obj)?;
    let snapshot_dtype = snapshot.getattr("dtype")?;
    let snapshot = if snapshot_dtype.getattr("isnative")?.extract::<bool>()? {
        snapshot
    } else {
        let native_dtype = snapshot_dtype.call_method1("newbyteorder", ("=",))?;
        let kwargs = PyDict::new(obj.py());
        kwargs.set_item("dtype", native_dtype)?;
        obj.py()
            .import("numpy")?
            .getattr("ascontiguousarray")?
            .call((snapshot,), Some(&kwargs))?
    };

    let dtype = snapshot.getattr("dtype")?.str()?.to_str()?.to_owned();
    if dtype == "float16" {
        let bits = snapshot.call_method1("view", ("uint16",))?;
        let array = bits.extract::<PyReadonlyArrayDyn<'_, u16>>()?;
        return dense_core_from_numpy_snapshot(array, ArrayStorage::Float16);
    }

    macro_rules! extract {
        ($rust:ty, $storage:ident) => {
            if let Ok(array) = snapshot.extract::<PyReadonlyArrayDyn<'_, $rust>>() {
                return dense_core_from_numpy_snapshot(array, ArrayStorage::$storage);
            }
        };
    }

    extract!(f64, Float64);
    extract!(f32, Float32);
    extract!(i64, Int64);
    extract!(i32, Int32);
    extract!(u32, Uint32);
    extract!(u16, Uint16);
    extract!(u8, Uint8);
    extract!(bool, Bool);

    Err(PyTypeError::new_err(format!(
        "NumPy dtype '{dtype}' is not supported; expected float16, float32, float64, \
         int32, int64, uint8, uint16, uint32, or bool"
    )))
}

/// Snapshot a NumPy scalar while preserving its exact supported dtype.
fn array_from_numpy_scalar(obj: &Bound<'_, PyAny>) -> PyResult<Option<DenseArrayCore>> {
    let Some(numpy) = loaded_numpy(obj.py())? else {
        return Ok(None);
    };
    let numpy_generic = numpy.getattr("generic")?;
    if !obj.is_instance(&numpy_generic)? {
        return Ok(None);
    }

    macro_rules! extract {
        ($dtype:ident, $name:literal, $rust:ty, $storage:ident) => {
            if $dtype == $name {
                let value = obj.extract::<$rust>()?;
                return DenseArrayCore::from_storage(ArrayStorage::$storage(vec![value]), &[])
                    .map(Some)
                    .map_err(map_array_err);
            }
        };
    }

    let dtype_string = obj.getattr("dtype")?.str()?;
    let dtype = dtype_string.to_str()?;
    if dtype == "float16" {
        let scalar_array = numpy.getattr("asarray")?.call1((obj,))?;
        let bits = scalar_array.call_method1("view", ("uint16",))?;
        let array = bits.extract::<PyReadonlyArrayDyn<'_, u16>>()?;
        let value = *array
            .as_array()
            .iter()
            .next()
            .expect("a NumPy scalar snapshot has one element");
        return DenseArrayCore::from_storage(ArrayStorage::Float16(vec![value]), &[])
            .map(Some)
            .map_err(map_array_err);
    }
    extract!(dtype, "float64", f64, Float64);
    extract!(dtype, "float32", f32, Float32);
    extract!(dtype, "int64", i64, Int64);
    extract!(dtype, "int32", i32, Int32);
    extract!(dtype, "uint32", u32, Uint32);
    extract!(dtype, "uint16", u16, Uint16);
    extract!(dtype, "uint8", u8, Uint8);
    extract!(dtype, "bool", bool, Bool);

    Err(PyTypeError::new_err(format!(
        "NumPy dtype '{dtype}' is not supported; expected float16, float32, float64, \
         int32, int64, uint8, uint16, uint32, or bool"
    )))
}

/// Parse a shape argument: an int or a sequence of ints.
pub fn parse_shape(obj: &Bound<'_, PyAny>) -> PyResult<Vec<usize>> {
    if let Ok(n) = obj.extract::<usize>() {
        return Ok(vec![n]);
    }
    if is_sequence(obj) {
        let seq = obj.cast::<PySequence>()?;
        let len = seq.len()?;
        if len > MAX_NDIM {
            return Err(PyValueError::new_err(format!(
                "maximum supported array dimensionality is {MAX_NDIM}, got {len}"
            )));
        }
        let mut shape = Vec::with_capacity(len);
        for i in 0..len {
            shape.push(seq.get_item(i)?.extract::<usize>()?);
        }
        checked_num_elements(&shape).map_err(map_array_err)?;
        return Ok(shape);
    }
    Err(PyTypeError::new_err(
        "shape must be an int or a sequence of ints",
    ))
}

fn nested(
    py: Python<'_>,
    shape: &[usize],
    scalars: &[Scalar],
    pos: &mut usize,
) -> PyResult<Py<PyAny>> {
    if shape.is_empty() {
        let value = scalars[*pos];
        *pos += 1;
        return Ok(scalar_to_py(py, value));
    }
    let list = PyList::empty(py);
    for _ in 0..shape[0] {
        list.append(nested(py, &shape[1..], scalars, pos)?)?;
    }
    Ok(list.into_any().unbind())
}

/// `tolist()`: nested Python lists (or a scalar for 0-d arrays).
pub fn to_list(py: Python<'_>, core: &DenseArrayCore) -> PyResult<Py<PyAny>> {
    let scalars = core.to_scalars().map_err(map_array_err)?;
    let mut pos = 0;
    nested(py, core.shape(), &scalars, &mut pos)
}

fn reshaped<T: numpy::Element>(
    py: Python<'_>,
    data: Vec<T>,
    shape: &[usize],
) -> PyResult<Py<PyAny>> {
    let flat = PyArray1::from_vec(py, data);
    let dims = shape.to_vec();
    Ok(flat.reshape(dims)?.into_any().unbind())
}

fn reshaped_float16_bits(py: Python<'_>, data: Vec<u16>, shape: &[usize]) -> PyResult<Py<PyAny>> {
    let bits = reshaped(py, data, shape)?;
    let float16 = py.import("numpy")?.getattr("float16")?;
    Ok(bits.bind(py).call_method1("view", (float16,))?.unbind())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypedCopyPath {
    Contiguous,
    Strided,
}

fn copy_selected<T: Copy>(values: &[T], layout: &Layout) -> (Vec<T>, TypedCopyPath) {
    if layout.is_c_contiguous() {
        let start = layout.offset;
        let end = start + layout.num_elements();
        return (values[start..end].to_vec(), TypedCopyPath::Contiguous);
    }
    (
        layout.iter_offsets().map(|offset| values[offset]).collect(),
        TypedCopyPath::Strided,
    )
}

/// Copy the bounded array into an owned real NumPy array of the same dtype.
pub fn to_numpy(py: Python<'_>, core: &DenseArrayCore) -> PyResult<Py<PyAny>> {
    // Fail with Python's ordinary ModuleNotFoundError before the rust-numpy
    // adapter tries to initialize its C API.
    py.import("numpy")?;
    let shape = core.shape();
    let layout = core.layout();
    let storage = core.read_storage().map_err(map_array_err)?;
    macro_rules! export {
        ($values:expr) => {{
            let (data, _) = copy_selected($values, layout);
            drop(storage);
            return reshaped(py, data, shape);
        }};
    }
    macro_rules! export_float16 {
        ($values:expr) => {{
            let (data, _) = copy_selected($values, layout);
            drop(storage);
            return reshaped_float16_bits(py, data, shape);
        }};
    }
    match &*storage {
        ArrayStorage::Float16(values) => export_float16!(values),
        ArrayStorage::Float32(values) => export!(values),
        ArrayStorage::Float64(values) => export!(values),
        ArrayStorage::Int64(values) => export!(values),
        ArrayStorage::Int32(values) => export!(values),
        ArrayStorage::Uint32(values) => export!(values),
        ArrayStorage::Uint16(values) => export!(values),
        ArrayStorage::Uint8(values) => export!(values),
        ArrayStorage::Bool(values) => export!(values),
        ArrayStorage::BorrowedF32 { .. } | ArrayStorage::BorrowedMutF32 { .. } => {
            let values = storage
                .f32_contiguous()
                .expect("f32 borrow has contiguous typed storage");
            export!(values)
        }
        ArrayStorage::BorrowedU8 { .. } | ArrayStorage::BorrowedMutU8 { .. } => {
            let values = storage
                .u8_contiguous()
                .expect("u8 borrow has contiguous typed storage");
            export!(values)
        }
    }
}
