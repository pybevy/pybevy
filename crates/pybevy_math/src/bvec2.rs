use bevy::math::BVec2;
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    public_error::{BVEC2_INDEX_OUT_OF_RANGE, UNSUPPORTED_COMPARISON},
};
use pybevy_macros::pyvalue;
use pyo3::{
    basic::CompareOp,
    exceptions::{PyIndexError, PyTypeError},
    prelude::*,
};

use crate::richcmp::comparison_result;

fn py_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

#[pyvalue]
#[pyclass(name = "BVec2", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyBVec2 {
    pub(crate) storage: ValueStorage<BVec2>,
}

impl From<BVec2> for PyBVec2 {
    fn from(value: BVec2) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyBVec2> for BVec2 {
    type Error = PyErr;

    fn try_from(value: PyBVec2) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyBVec2> for BVec2 {
    type Error = PyErr;

    fn try_from(value: &PyBVec2) -> PyResult<Self> {
        value.to_bevy()
    }
}

fn check_index(index: usize) -> PyResult<()> {
    if index > 1 {
        return Err(PyIndexError::new_err(BVEC2_INDEX_OUT_OF_RANGE));
    }
    Ok(())
}

#[pymethods]
impl PyBVec2 {
    #[new]
    #[pyo3(signature = (x=false, y=false))]
    pub fn new(x: bool, y: bool) -> Self {
        Self::from_owned(BVec2::new(x, y))
    }

    #[staticmethod]
    #[pyo3(name = "FALSE")]
    pub fn false_constant() -> Self {
        Self::from_owned(BVec2::FALSE)
    }

    #[staticmethod]
    #[pyo3(name = "TRUE")]
    pub fn true_constant() -> Self {
        Self::from_owned(BVec2::TRUE)
    }

    #[staticmethod]
    pub fn splat(v: bool) -> Self {
        Self::from_owned(BVec2::splat(v))
    }

    #[staticmethod]
    pub fn from_array(a: [bool; 2]) -> Self {
        Self::from_owned(BVec2::from_array(a))
    }

    #[getter]
    pub fn x(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, x: bool) -> PyResult<()> {
        self.storage.update(|value| value.x = x)?;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, y: bool) -> PyResult<()> {
        self.storage.update(|value| value.y = y)?;
        Ok(())
    }

    pub fn bitmask(&self) -> PyResult<u32> {
        Ok(self.to_bevy()?.bitmask())
    }

    pub fn any(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.any())
    }

    pub fn all(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.all())
    }

    pub fn test(&self, index: usize) -> PyResult<bool> {
        check_index(index)?;
        Ok(self.to_bevy()?.test(index))
    }

    pub fn set(&mut self, index: usize, value: bool) -> PyResult<()> {
        check_index(index)?;
        self.storage.update(|mask| mask.set(index, value))?;
        Ok(())
    }

    pub fn __and__(&self, other: PyRef<'_, Self>) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()? & other.to_bevy()?))
    }

    pub fn __or__(&self, other: PyRef<'_, Self>) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()? | other.to_bevy()?))
    }

    pub fn __xor__(&self, other: PyRef<'_, Self>) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()? ^ other.to_bevy()?))
    }

    pub fn __invert__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(!self.to_bevy()?))
    }

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other) = other.extract::<PyRef<'_, Self>>() else {
            return Ok(py.NotImplemented());
        };
        let (a, b) = (self.to_bevy()?, other.to_bevy()?);
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }

    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let value = self.to_bevy()?;
        Ok(format!("BVec2({}, {})", py_bool(value.x), py_bool(value.y)))
    }
}
