use bevy::math::IVec2;
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyOverflowError, PyTypeError, PyZeroDivisionError},
    prelude::*,
};

#[pyclass(name = "IVec2", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyIVec2 {
    storage: ValueStorage<IVec2>,
}

impl TryFrom<PyIVec2> for IVec2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_vec: PyIVec2) -> PyResult<Self> {
        Ok(py_vec.storage.get()?)
    }
}

impl TryFrom<&PyIVec2> for IVec2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_vec: &PyIVec2) -> PyResult<Self> {
        Ok(py_vec.storage.get()?)
    }
}

impl From<IVec2> for PyIVec2 {
    #[inline(always)]
    fn from(vec: IVec2) -> Self {
        PyIVec2::from_ivec2(vec)
    }
}

impl FromBorrowedStorage<ValueStorage<IVec2>> for PyIVec2 {
    fn from_borrowed(storage: ValueStorage<IVec2>) -> Self {
        PyIVec2 { storage }
    }
}

impl PartialEq for PyIVec2 {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl PyIVec2 {
    #[inline(always)]
    pub fn from_ivec2(vec: IVec2) -> Self {
        PyIVec2 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    pub const fn ivec2(vec: IVec2) -> Self {
        PyIVec2 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, IVec2>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, IVec2>> {
        Ok(self.storage.as_mut()?)
    }

    pub const ZERO: PyIVec2 = PyIVec2::ivec2(IVec2::ZERO);
    pub const ONE: PyIVec2 = PyIVec2::ivec2(IVec2::ONE);
    pub const NEG_ONE: PyIVec2 = PyIVec2::ivec2(IVec2::NEG_ONE);
    pub const X: PyIVec2 = PyIVec2::ivec2(IVec2::X);
    pub const Y: PyIVec2 = PyIVec2::ivec2(IVec2::Y);
    pub const NEG_X: PyIVec2 = PyIVec2::ivec2(IVec2::NEG_X);
    pub const NEG_Y: PyIVec2 = PyIVec2::ivec2(IVec2::NEG_Y);
    pub const MIN: PyIVec2 = PyIVec2::ivec2(IVec2::MIN);
    pub const MAX: PyIVec2 = PyIVec2::ivec2(IVec2::MAX);
}

#[pymethods]
impl PyIVec2 {
    #[new]
    pub fn new(x: i32, y: i32) -> Self {
        PyIVec2::from_ivec2(IVec2::new(x, y))
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::ivec2(IVec2::ZERO)
    }
    #[staticmethod]
    #[pyo3(name = "ONE")]
    pub fn one() -> Self {
        Self::ivec2(IVec2::ONE)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_ONE")]
    pub fn neg_one() -> Self {
        Self::ivec2(IVec2::NEG_ONE)
    }
    #[staticmethod]
    #[pyo3(name = "X")]
    pub fn unit_x() -> Self {
        Self::ivec2(IVec2::X)
    }
    #[staticmethod]
    #[pyo3(name = "Y")]
    pub fn unit_y() -> Self {
        Self::ivec2(IVec2::Y)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_X")]
    pub fn neg_x() -> Self {
        Self::ivec2(IVec2::NEG_X)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_Y")]
    pub fn neg_y() -> Self {
        Self::ivec2(IVec2::NEG_Y)
    }
    #[staticmethod]
    #[pyo3(name = "MIN")]
    pub fn min_value() -> Self {
        Self::ivec2(IVec2::MIN)
    }
    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max_value() -> Self {
        Self::ivec2(IVec2::MAX)
    }

    #[getter]
    pub fn x(&self) -> PyResult<i32> {
        Ok(self.as_ref()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, value: i32) -> PyResult<()> {
        self.as_mut()?.x = value;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<i32> {
        Ok(self.as_ref()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, value: i32) -> PyResult<()> {
        self.as_mut()?.y = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let vec = self.as_ref()?;
        Ok(format!("IVec2(x={}, y={})", vec.x, vec.y))
    }

    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_vec) = other.extract::<PyIVec2>() {
            let a = self.as_ref()?;
            let b = other_vec.as_ref()?;
            match op {
                CompareOp::Eq => Ok(a == b),
                CompareOp::Ne => Ok(a != b),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare IVec2 with another IVec2",
            ))
        }
    }

    pub fn __add__(&self, other: &PyIVec2) -> PyResult<PyIVec2> {
        Ok(PyIVec2::from_ivec2(*self.as_ref()? + *other.as_ref()?))
    }

    pub fn __sub__(&self, other: &PyIVec2) -> PyResult<PyIVec2> {
        Ok(PyIVec2::from_ivec2(*self.as_ref()? - *other.as_ref()?))
    }

    pub fn __mul__(&self, scalar: i32) -> PyResult<PyIVec2> {
        Ok(PyIVec2::from_ivec2(*self.as_ref()? * scalar))
    }

    pub fn __truediv__(&self, scalar: i32) -> PyResult<PyIVec2> {
        if scalar == 0 {
            return Err(PyZeroDivisionError::new_err("IVec2 division by zero"));
        }
        if scalar == -1 && *self.as_ref()? == IVec2::MIN {
            return Err(PyOverflowError::new_err(
                "IVec2 division overflows: i32::MIN / -1",
            ));
        }
        Ok(PyIVec2::from_ivec2(*self.as_ref()? / scalar))
    }

    pub fn __neg__(&self) -> PyResult<PyIVec2> {
        Ok(PyIVec2::from_ivec2(-*self.as_ref()?))
    }

    #[staticmethod]
    pub fn splat(value: i32) -> PyIVec2 {
        PyIVec2::from_ivec2(IVec2::splat(value))
    }

    pub fn min(&self, other: &PyIVec2) -> PyResult<PyIVec2> {
        Ok(PyIVec2::from_ivec2(self.as_ref()?.min(*other.as_ref()?)))
    }

    pub fn max(&self, other: &PyIVec2) -> PyResult<PyIVec2> {
        Ok(PyIVec2::from_ivec2(self.as_ref()?.max(*other.as_ref()?)))
    }

    pub fn abs(&self) -> PyResult<PyIVec2> {
        Ok(PyIVec2::from_ivec2(self.as_ref()?.abs()))
    }

    pub fn signum(&self) -> PyResult<PyIVec2> {
        Ok(PyIVec2::from_ivec2(self.as_ref()?.signum()))
    }

    pub fn dot(&self, other: &PyIVec2) -> PyResult<i32> {
        Ok(self.as_ref()?.dot(*other.as_ref()?))
    }

    pub fn length_squared(&self) -> PyResult<i32> {
        Ok(self.as_ref()?.length_squared())
    }
}
