//! Python <-> bounded array conversion: nested-list construction, `tolist`,
//! shape parsing, and copy-based real-NumPy interop.

use numpy::{PyArray1, PyArrayMethods};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyList, PySequence, PyString, PyTuple},
};

use super::{
    array::PyArray,
    kernels::{extract_scalar, map_array_err, scalar_to_py},
};
use crate::{ArrayDType, ArrayStorage, DenseArrayCore, MAX_NDIM, Scalar, checked_num_elements};

fn is_sequence(obj: &Bound<'_, PyAny>) -> bool {
    if obj.is_instance_of::<PyString>() {
        return false;
    }
    obj.is_instance_of::<PyList>() || obj.is_instance_of::<PyTuple>()
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
        let core = nd.core.clone();
        return match dtype {
            Some(dt) if dt != core.dtype() => core.astype(dt).map_err(map_array_err),
            _ => core.copy().map_err(map_array_err),
        };
    }
    let (shape, flat) = collect(obj, 0)?;
    let dtype = dtype.unwrap_or_else(|| infer_dtype(&flat));
    let storage = storage_from_scalars(&flat, dtype)?;
    DenseArrayCore::from_storage(storage, &shape).map_err(map_array_err)
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

/// Copy the bounded array into an owned real NumPy array of the same dtype.
pub fn to_numpy(py: Python<'_>, core: &DenseArrayCore) -> PyResult<Py<PyAny>> {
    let scalars = core.to_scalars().map_err(map_array_err)?;
    let shape = core.shape();
    match core.dtype() {
        ArrayDType::Float64 => reshaped(py, scalars.iter().map(|s| s.to_f64()).collect(), shape),
        ArrayDType::Float32 => reshaped(
            py,
            scalars.iter().map(|s| s.to_f64() as f32).collect(),
            shape,
        ),
        ArrayDType::Int64 => reshaped(
            py,
            scalars.iter().map(|s| s.to_i64_trunc()).collect(),
            shape,
        ),
        ArrayDType::Int32 => reshaped(
            py,
            scalars.iter().map(|s| s.to_i64_trunc() as i32).collect(),
            shape,
        ),
        ArrayDType::Uint32 => reshaped(
            py,
            scalars.iter().map(|s| s.to_i64_trunc() as u32).collect(),
            shape,
        ),
        ArrayDType::Uint16 => reshaped(
            py,
            scalars.iter().map(|s| s.to_i64_trunc() as u16).collect(),
            shape,
        ),
        ArrayDType::Uint8 => reshaped(
            py,
            scalars.iter().map(|s| s.to_i64_trunc() as u8).collect(),
            shape,
        ),
        ArrayDType::Bool => reshaped(py, scalars.iter().map(|s| s.to_bool()).collect(), shape),
    }
}
