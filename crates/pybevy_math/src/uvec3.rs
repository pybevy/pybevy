use bevy::math::UVec3;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

#[pyclass(name = "UVec3", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyUVec3 {
    storage: ValueStorage<UVec3>,
}

impl From<PyUVec3> for UVec3 {
    #[inline(always)]
    fn from(py_vec: PyUVec3) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<&PyUVec3> for UVec3 {
    #[inline(always)]
    fn from(py_vec: &PyUVec3) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<UVec3> for PyUVec3 {
    #[inline(always)]
    fn from(vec: UVec3) -> Self {
        PyUVec3::from_uvec3(vec)
    }
}

impl FromBorrowedStorage<ValueStorage<UVec3>> for PyUVec3 {
    fn from_borrowed(storage: ValueStorage<UVec3>) -> Self {
        PyUVec3 { storage }
    }
}

impl PyUVec3 {
    #[inline(always)]
    pub fn from_uvec3(vec: UVec3) -> Self {
        PyUVec3 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    pub const fn uvec3(vec: UVec3) -> Self {
        PyUVec3 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&UVec3> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut UVec3> {
        Ok(self.storage.as_mut()?)
    }

    pub const ZERO: PyUVec3 = PyUVec3::uvec3(UVec3::ZERO);
    pub const ONE: PyUVec3 = PyUVec3::uvec3(UVec3::ONE);
    pub const X: PyUVec3 = PyUVec3::uvec3(UVec3::X);
    pub const Y: PyUVec3 = PyUVec3::uvec3(UVec3::Y);
    pub const Z: PyUVec3 = PyUVec3::uvec3(UVec3::Z);
    pub const MIN: PyUVec3 = PyUVec3::uvec3(UVec3::MIN);
    pub const MAX: PyUVec3 = PyUVec3::uvec3(UVec3::MAX);
}

#[pymethods]
impl PyUVec3 {
    #[new]
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        PyUVec3::from_uvec3(UVec3::new(x, y, z))
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::uvec3(UVec3::ZERO)
    }
    #[staticmethod]
    #[pyo3(name = "ONE")]
    pub fn one() -> Self {
        Self::uvec3(UVec3::ONE)
    }
    #[staticmethod]
    #[pyo3(name = "X")]
    pub fn unit_x() -> Self {
        Self::uvec3(UVec3::X)
    }
    #[staticmethod]
    #[pyo3(name = "Y")]
    pub fn unit_y() -> Self {
        Self::uvec3(UVec3::Y)
    }
    #[staticmethod]
    #[pyo3(name = "Z")]
    pub fn unit_z() -> Self {
        Self::uvec3(UVec3::Z)
    }
    #[staticmethod]
    #[pyo3(name = "MIN")]
    pub fn min_value() -> Self {
        Self::uvec3(UVec3::MIN)
    }
    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max_value() -> Self {
        Self::uvec3(UVec3::MAX)
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

    #[getter]
    pub fn z(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.z)
    }

    #[setter]
    pub fn set_z(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.z = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let vec = self.as_ref()?;
        Ok(format!("UVec3({}, {}, {})", vec.x, vec.y, vec.z))
    }

    pub fn __richcmp__(&self, other: &PyUVec3, op: CompareOp) -> PyResult<bool> {
        let a = self.as_ref()?;
        let b = other.as_ref()?;
        match op {
            CompareOp::Eq => Ok(a == b),
            CompareOp::Ne => Ok(a != b),
            _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
        }
    }

    pub fn __add__(&self, other: &PyUVec3) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(*self.as_ref()? + *other.as_ref()?))
    }

    pub fn __sub__(&self, other: &PyUVec3) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(*self.as_ref()? - *other.as_ref()?))
    }

    pub fn __mul__(&self, scalar: u32) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(*self.as_ref()? * scalar))
    }

    pub fn __truediv__(&self, scalar: u32) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(*self.as_ref()? / scalar))
    }

    #[staticmethod]
    pub fn splat(value: u32) -> PyUVec3 {
        PyUVec3::from_uvec3(UVec3::splat(value))
    }

    pub fn min(&self, other: &PyUVec3) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(self.as_ref()?.min(*other.as_ref()?)))
    }

    pub fn max(&self, other: &PyUVec3) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(self.as_ref()?.max(*other.as_ref()?)))
    }

    pub fn dot(&self, other: &PyUVec3) -> PyResult<u32> {
        Ok(self.as_ref()?.dot(*other.as_ref()?))
    }

    pub fn length_squared(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.length_squared())
    }
}
