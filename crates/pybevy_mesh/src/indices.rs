use bevy::mesh::Indices;
use numpy::PyReadonlyArray1;
use pybevy_array::{
    ArrayDType, ArrayStorage, PyArray, owned_contiguous_le_bytes, read_only_storage,
};
use pybevy_core::public_error;
use pybevy_macros::pyenum;
use pyo3::{
    exceptions::{PyOverflowError, PyTypeError},
    prelude::*,
    types::{PyAny, PyBytes, PyList, PyString, PyTuple},
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
            ArrayDType::Uint16 => Ok(Indices::U16(le_u16(&bytes))),
            ArrayDType::Uint32 => Ok(Indices::U32(le_u32(&bytes))),
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

fn le_u16(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

fn le_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn variant_values(value: &Bound<'_, PyAny>, name: &str, accept_u32: bool) -> PyResult<Vec<u32>> {
    let invalid = || PyTypeError::new_err(public_error::indices_variant_values(name));
    if let Ok(array) = value.extract::<PyRef<PyArray>>() {
        let (dtype, shape, bytes) = owned_contiguous_le_bytes(&array)?;
        return match (dtype, shape.len()) {
            (ArrayDType::Uint16, 1) => Ok(le_u16(&bytes).into_iter().map(u32::from).collect()),
            (ArrayDType::Uint32, 1) if accept_u32 => Ok(le_u32(&bytes)),
            _ => Err(invalid()),
        };
    }
    if value.is_instance_of::<PyString>() || value.is_instance_of::<PyBytes>() {
        return Err(invalid());
    }
    value.extract::<Vec<u32>>().map_err(|_| invalid())
}

/// Bevy reserves `u16::MAX` as the primitive-restart value of `Indices::U16`.
fn u16_index(index: u32) -> PyResult<u16> {
    u16::try_from(index)
        .ok()
        .filter(|&index| index < u16::MAX)
        .ok_or_else(|| PyOverflowError::new_err(public_error::indices_u16_overflow(index)))
}

fn u16_values(value: &Bound<'_, PyAny>) -> PyResult<Vec<u16>> {
    variant_values(value, "U16", true)?
        .into_iter()
        .map(u16_index)
        .collect()
}

fn u32_values(value: &Bound<'_, PyAny>) -> PyResult<Vec<u32>> {
    variant_values(value, "U32", true)
}

#[pyenum(Indices, manual)]
#[pyclass(
    name = "Indices",
    module = "pybevy.mesh",
    eq,
    subclass,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyIndices {
    pub(crate) inner: Indices,
}

impl PyIndices {
    fn extend_values(&mut self, values: Vec<u32>) -> PyResult<()> {
        if let Indices::U16(_) = self.inner {
            for &index in &values {
                u16_index(index)?;
            }
        }
        self.inner.extend(values);
        Ok(())
    }

    fn values(&self) -> Vec<usize> {
        self.inner.iter().collect()
    }
}

#[pymethods]
impl PyIndices {
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn push(&mut self, index: u32) -> PyResult<()> {
        self.extend_values(vec![index])
    }

    pub fn extend(&mut self, values: Vec<u32>) -> PyResult<()> {
        self.extend_values(values)
    }

    pub fn iter(&self) -> Vec<usize> {
        self.values()
    }

    pub fn __iter__(&self) -> PyIndicesIterator {
        PyIndicesIterator {
            values: self.values(),
            index: 0,
        }
    }

    fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        materialize_indices(py, self.inner.clone())
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<Self>> {
        self.__copy__(py)
    }

    fn __repr__(&self) -> String {
        let values: Vec<String> = self.inner.iter().map(|index| index.to_string()).collect();
        let variant = match self.inner {
            Indices::U16(_) => "U16",
            Indices::U32(_) => "U32",
        };
        format!("Indices.{variant}([{}])", values.join(", "))
    }
}

#[pyclass(name = "U16", module = "pybevy.mesh", extends = PyIndices)]
pub struct PyIndicesU16;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyIndicesU16 {
    #[classattr]
    const __qualname__: &'static str = "Indices.U16";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let values = u16_values(value)?;
        Ok(PyClassInitializer::from(PyIndices {
            inner: Indices::U16(values),
        })
        .add_subclass(Self))
    }

    #[getter]
    fn value(slf: PyRef<'_, Self>) -> PyResult<PyArray> {
        let base = slf.into_super();
        let Indices::U16(values) = &base.inner else {
            unreachable!("Indices.U16 holds U16 storage")
        };
        read_only_storage(ArrayStorage::Uint16(values.clone()), &[values.len()])
    }
}

#[pyclass(name = "U32", module = "pybevy.mesh", extends = PyIndices)]
pub struct PyIndicesU32;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyIndicesU32 {
    #[classattr]
    const __qualname__: &'static str = "Indices.U32";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let values = u32_values(value)?;
        Ok(PyClassInitializer::from(PyIndices {
            inner: Indices::U32(values),
        })
        .add_subclass(Self))
    }

    #[getter]
    fn value(slf: PyRef<'_, Self>) -> PyResult<PyArray> {
        let base = slf.into_super();
        let Indices::U32(values) = &base.inner else {
            unreachable!("Indices.U32 holds U32 storage")
        };
        read_only_storage(ArrayStorage::Uint32(values.clone()), &[values.len()])
    }
}

pub fn materialize_indices(py: Python<'_>, inner: Indices) -> PyResult<Py<PyIndices>> {
    let instance = match inner {
        Indices::U16(_) => Py::new(
            py,
            PyClassInitializer::from(PyIndices { inner }).add_subclass(PyIndicesU16),
        )?
        .into_bound(py)
        .into_super()
        .unbind(),
        Indices::U32(_) => Py::new(
            py,
            PyClassInitializer::from(PyIndices { inner }).add_subclass(PyIndicesU32),
        )?
        .into_bound(py)
        .into_super()
        .unbind(),
    };
    Ok(instance)
}

pub fn register_indices_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("Indices")?;
    base.setattr("U16", py.get_type::<PyIndicesU16>())?;
    base.setattr("U32", py.get_type::<PyIndicesU32>())?;
    Ok(())
}

#[pyclass(name = "IndicesIterator", module = "pybevy.mesh")]
pub struct PyIndicesIterator {
    values: Vec<usize>,
    index: usize,
}

#[pymethods]
impl PyIndicesIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(&mut self) -> Option<usize> {
        let value = self.values.get(self.index).copied();
        self.index += usize::from(value.is_some());
        value
    }
}
