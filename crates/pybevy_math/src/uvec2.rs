use bevy::math::UVec2;
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyZeroDivisionError},
    prelude::*,
};

#[pyclass(name = "UVec2", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyUVec2 {
    storage: ValueStorage<UVec2>,
}

impl TryFrom<PyUVec2> for UVec2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_vec: PyUVec2) -> PyResult<Self> {
        Ok(py_vec.storage.get()?)
    }
}

impl TryFrom<&PyUVec2> for UVec2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_vec: &PyUVec2) -> PyResult<Self> {
        Ok(py_vec.storage.get()?)
    }
}

impl From<UVec2> for PyUVec2 {
    #[inline(always)]
    fn from(vec: UVec2) -> Self {
        PyUVec2::from_uvec2(vec)
    }
}

impl FromBorrowedStorage<ValueStorage<UVec2>> for PyUVec2 {
    fn from_borrowed(storage: ValueStorage<UVec2>) -> Self {
        PyUVec2 { storage }
    }
}

impl PartialEq for PyUVec2 {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl PyUVec2 {
    #[inline(always)]
    pub fn from_uvec2(vec: UVec2) -> Self {
        PyUVec2 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    pub const fn uvec2(vec: UVec2) -> Self {
        PyUVec2 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, UVec2>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, UVec2>> {
        Ok(self.storage.as_mut()?)
    }

    pub const ZERO: PyUVec2 = PyUVec2::uvec2(UVec2::ZERO);
    pub const ONE: PyUVec2 = PyUVec2::uvec2(UVec2::ONE);
    pub const X: PyUVec2 = PyUVec2::uvec2(UVec2::X);
    pub const Y: PyUVec2 = PyUVec2::uvec2(UVec2::Y);
    pub const MIN: PyUVec2 = PyUVec2::uvec2(UVec2::MIN);
    pub const MAX: PyUVec2 = PyUVec2::uvec2(UVec2::MAX);
}

#[pymethods]
impl PyUVec2 {
    #[new]
    pub fn new(x: u32, y: u32) -> Self {
        PyUVec2::from_uvec2(UVec2::new(x, y))
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::uvec2(UVec2::ZERO)
    }
    #[staticmethod]
    #[pyo3(name = "ONE")]
    pub fn one() -> Self {
        Self::uvec2(UVec2::ONE)
    }
    #[staticmethod]
    #[pyo3(name = "X")]
    pub fn unit_x() -> Self {
        Self::uvec2(UVec2::X)
    }
    #[staticmethod]
    #[pyo3(name = "Y")]
    pub fn unit_y() -> Self {
        Self::uvec2(UVec2::Y)
    }
    #[staticmethod]
    #[pyo3(name = "MIN")]
    pub fn min_value() -> Self {
        Self::uvec2(UVec2::MIN)
    }
    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max_value() -> Self {
        Self::uvec2(UVec2::MAX)
    }

    #[getter]
    pub fn x(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.x = value;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.y = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let vec = self.as_ref()?;
        Ok(format!("UVec2(x={}, y={})", vec.x, vec.y))
    }

    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_vec) = other.extract::<PyUVec2>() {
            let a = self.as_ref()?;
            let b = other_vec.as_ref()?;
            match op {
                CompareOp::Eq => Ok(a == b),
                CompareOp::Ne => Ok(a != b),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare UVec2 with another UVec2",
            ))
        }
    }

    pub fn __add__(&self, other: &PyUVec2) -> PyResult<PyUVec2> {
        Ok(PyUVec2::from_uvec2(*self.as_ref()? + *other.as_ref()?))
    }

    pub fn __sub__(&self, other: &PyUVec2) -> PyResult<PyUVec2> {
        Ok(PyUVec2::from_uvec2(*self.as_ref()? - *other.as_ref()?))
    }

    pub fn __mul__(&self, scalar: u32) -> PyResult<PyUVec2> {
        Ok(PyUVec2::from_uvec2(*self.as_ref()? * scalar))
    }

    pub fn __truediv__(&self, scalar: u32) -> PyResult<PyUVec2> {
        if scalar == 0 {
            return Err(PyZeroDivisionError::new_err("UVec2 division by zero"));
        }
        Ok(PyUVec2::from_uvec2(*self.as_ref()? / scalar))
    }

    #[staticmethod]
    pub fn splat(value: u32) -> PyUVec2 {
        PyUVec2::from_uvec2(UVec2::splat(value))
    }

    pub fn min(&self, other: &PyUVec2) -> PyResult<PyUVec2> {
        Ok(PyUVec2::from_uvec2(self.as_ref()?.min(*other.as_ref()?)))
    }

    pub fn max(&self, other: &PyUVec2) -> PyResult<PyUVec2> {
        Ok(PyUVec2::from_uvec2(self.as_ref()?.max(*other.as_ref()?)))
    }

    pub fn dot(&self, other: &PyUVec2) -> PyResult<u32> {
        Ok(self.as_ref()?.dot(*other.as_ref()?))
    }

    pub fn length_squared(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.length_squared())
    }

    pub fn min_element(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.min_element())
    }

    pub fn max_element(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.max_element())
    }

    pub fn element_sum(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.element_sum())
    }

    pub fn element_product(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.element_product())
    }

    pub fn with_x(&self, x: u32) -> PyResult<PyUVec2> {
        Ok(PyUVec2::from_uvec2(self.as_ref()?.with_x(x)))
    }

    pub fn with_y(&self, y: u32) -> PyResult<PyUVec2> {
        Ok(PyUVec2::from_uvec2(self.as_ref()?.with_y(y)))
    }

    pub fn to_array(&self) -> PyResult<(u32, u32)> {
        let value = self.as_ref()?;
        Ok((value.x, value.y))
    }

    pub fn as_tuple(&self) -> PyResult<(u32, u32)> {
        self.to_array()
    }
}
