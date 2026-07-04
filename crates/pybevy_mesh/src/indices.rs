use numpy::PyReadonlyArray1;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyAny};

#[pyclass(name = "Indices", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyIndices {
    U16(Vec<u16>),
    U32(Vec<u32>),
}

#[pymethods]
impl PyIndices {
    #[new]
    fn new(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(arr_u32) = obj.extract::<PyReadonlyArray1<u32>>() {
            let slice = arr_u32.as_slice()?;
            Ok(PyIndices::U32(slice.to_vec()))
        } else if let Ok(arr_u16) = obj.extract::<PyReadonlyArray1<u16>>() {
            let slice = arr_u16.as_slice()?;
            Ok(PyIndices::U16(slice.to_vec()))
        } else if let Ok(vec32) = obj.extract::<Vec<u32>>() {
            Ok(PyIndices::U32(vec32))
        } else if let Ok(vec16) = obj.extract::<Vec<u16>>() {
            Ok(PyIndices::U16(vec16))
        } else {
            Err(PyTypeError::new_err(
                "Indices(...) expects a NumPy array of dtype uint32/uint16 or a Python sequence of ints",
            ))
        }
    }

    #[getter]
    pub fn len(&self) -> usize {
        match self {
            PyIndices::U16(vec) => vec.len(),
            PyIndices::U32(vec) => vec.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            PyIndices::U16(vec) => vec.is_empty(),
            PyIndices::U32(vec) => vec.is_empty(),
        }
    }

    pub fn __iter__(&self) -> PyResult<PyIndicesIterator> {
        Ok(PyIndicesIterator {
            indices: self.clone(),
            index: 0,
        })
    }
}

#[pyclass(name = "IndicesIterator")]
pub struct PyIndicesIterator {
    indices: PyIndices,
    index: usize,
}

#[pymethods]
impl PyIndicesIterator {
    pub fn __next__(&mut self) -> PyResult<Option<u32>> {
        Ok(match &self.indices {
            PyIndices::U16(vec) => {
                if self.index < vec.len() {
                    let value = vec[self.index] as u32;
                    self.index += 1;
                    Some(value)
                } else {
                    None
                }
            }
            PyIndices::U32(vec) => {
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
