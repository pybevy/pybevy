use bevy::mesh::Indices;
use numpy::PyReadonlyArray1;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyAny};

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
        let inner = if let Ok(arr_u32) = obj.extract::<PyReadonlyArray1<u32>>() {
            Indices::U32(arr_u32.as_slice()?.to_vec())
        } else if let Ok(arr_u16) = obj.extract::<PyReadonlyArray1<u16>>() {
            Indices::U16(arr_u16.as_slice()?.to_vec())
        } else if let Ok(vec32) = obj.extract::<Vec<u32>>() {
            Indices::U32(vec32)
        } else if let Ok(vec16) = obj.extract::<Vec<u16>>() {
            Indices::U16(vec16)
        } else {
            return Err(PyTypeError::new_err(
                "Indices(...) expects a NumPy array of dtype uint32/uint16 or a Python sequence of ints",
            ));
        };
        Ok(PyIndices { inner })
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
