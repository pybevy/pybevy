use bevy::mesh::Indices;
use numpy::PyReadonlyArray1;
use pybevy_array::{ArrayDType, PyArray, owned_contiguous_le_bytes};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyAny, PyList, PyTuple},
};

pub(crate) fn extract_indices(obj: &Bound<'_, PyAny>, error_message: &str) -> PyResult<Indices> {
    if let Ok(indices) = obj.extract::<PyRef<PyIndices>>() {
        return Ok(indices.inner.clone());
    }
    if let Ok(array) = obj.extract::<PyRef<PyArray>>() {
        let (dtype, shape, bytes) = owned_contiguous_le_bytes(&array)?;
        if shape.len() != 1 {
            return Err(PyTypeError::new_err(error_message.to_owned()));
        }
        return match dtype {
            ArrayDType::Uint16 => Ok(Indices::U16(
                bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect(),
            )),
            ArrayDType::Uint32 => Ok(Indices::U32(
                bytes
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect(),
            )),
            _ => Err(PyTypeError::new_err(error_message.to_owned())),
        };
    }
    if obj.is_instance_of::<PyList>() || obj.is_instance_of::<PyTuple>() {
        return obj
            .extract::<Vec<u32>>()
            .map(Indices::U32)
            .map_err(|_| PyTypeError::new_err(error_message.to_owned()));
    }
    if obj.py().import("numpy").is_err() {
        return Err(PyTypeError::new_err(error_message.to_owned()));
    }
    if let Ok(arr_u32) = obj.extract::<PyReadonlyArray1<u32>>() {
        return Ok(Indices::U32(arr_u32.as_slice()?.to_vec()));
    }
    if let Ok(arr_u16) = obj.extract::<PyReadonlyArray1<u16>>() {
        return Ok(Indices::U16(arr_u16.as_slice()?.to_vec()));
    }
    if let Ok(vec32) = obj.extract::<Vec<u32>>() {
        return Ok(Indices::U32(vec32));
    }
    if let Ok(vec16) = obj.extract::<Vec<u16>>() {
        return Ok(Indices::U16(vec16));
    }
    Err(PyTypeError::new_err(error_message.to_owned()))
}

/// Wraps bevy's `Indices` enum as a struct pyclass so `push` can mutate in
/// place (pyo3 freezes enum pyclasses).
///
/// Always an owned value: the mesh accepts Indices via `insert_indices` but
/// never hands out a copy, so a user-held Indices is unique and in-place
/// mutation is never silently lost.
///
/// WARNING: do not add a mesh getter returning a copy of `Indices`; that
/// would re-introduce silent mutation loss.
#[pyclass(name = "Indices", module = "pybevy.mesh", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyIndices {
    pub(crate) inner: Indices,
}

#[pymethods]
impl PyIndices {
    #[new]
    fn new(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let inner = extract_indices(
            obj,
            "Indices(...) expects a NumPy array of dtype uint32/uint16 or a Python sequence of ints",
        )?;
        Ok(PyIndices { inner })
    }

    fn __len__(&self) -> usize {
        self.len()
    }

    #[getter]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn push(&mut self, index: u32) {
        self.inner.push(index);
    }

    pub fn __iter__(&self) -> PyResult<PyIndicesIterator> {
        Ok(PyIndicesIterator {
            indices: self.clone(),
            index: 0,
        })
    }
}

#[pyclass(name = "IndicesIterator", module = "pybevy.mesh")]
pub struct PyIndicesIterator {
    indices: PyIndices,
    index: usize,
}

#[pymethods]
impl PyIndicesIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(&mut self) -> PyResult<Option<u32>> {
        Ok(match &self.indices.inner {
            Indices::U16(vec) => {
                if self.index < vec.len() {
                    let value = vec[self.index] as u32;
                    self.index += 1;
                    Some(value)
                } else {
                    None
                }
            }
            Indices::U32(vec) => {
                if self.index < vec.len() {
                    let value = vec[self.index];
                    self.index += 1;
                    Some(value)
                } else {
                    None
                }
            }
        })
    }
}

impl From<Indices> for PyIndices {
    fn from(inner: Indices) -> Self {
        PyIndices { inner }
    }
}
