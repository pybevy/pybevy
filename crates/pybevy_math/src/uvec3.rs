use bevy::math::UVec3;
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyZeroDivisionError},
    prelude::*,
};

use crate::{integer::checked, richcmp::comparison_result};

#[pyclass(name = "UVec3", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyUVec3 {
    storage: ValueStorage<UVec3>,
}

impl TryFrom<PyUVec3> for UVec3 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_vec: PyUVec3) -> PyResult<Self> {
        Ok(py_vec.storage.get()?)
    }
}

impl TryFrom<&PyUVec3> for UVec3 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_vec: &PyUVec3) -> PyResult<Self> {
        Ok(py_vec.storage.get()?)
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
    fn as_ref(&self) -> PyResult<StorageRef<'_, UVec3>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, UVec3>> {
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

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyUVec3>() else {
            return Ok(py.NotImplemented());
        };
        let a = self.as_ref()?;
        let b = other_value.as_ref()?;
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => return Err(PyTypeError::new_err("Unsupported comparison operation")),
        };
        Ok(comparison_result(py, result))
    }

    pub fn __add__(&self, other: &PyUVec3) -> PyResult<PyUVec3> {
        let value = self.as_ref()?;
        let other = other.as_ref()?;
        Ok(UVec3::new(
            checked(value.x.checked_add(other.x))?,
            checked(value.y.checked_add(other.y))?,
            checked(value.z.checked_add(other.z))?,
        )
        .into())
    }

    pub fn __sub__(&self, other: &PyUVec3) -> PyResult<PyUVec3> {
        let value = self.as_ref()?;
        let other = other.as_ref()?;
        Ok(UVec3::new(
            checked(value.x.checked_sub(other.x))?,
            checked(value.y.checked_sub(other.y))?,
            checked(value.z.checked_sub(other.z))?,
        )
        .into())
    }

    pub fn __mul__(&self, scalar: u32) -> PyResult<PyUVec3> {
        let value = self.as_ref()?;
        Ok(UVec3::new(
            checked(value.x.checked_mul(scalar))?,
            checked(value.y.checked_mul(scalar))?,
            checked(value.z.checked_mul(scalar))?,
        )
        .into())
    }

    pub fn __truediv__(&self, scalar: u32) -> PyResult<PyUVec3> {
        if scalar == 0 {
            return Err(PyZeroDivisionError::new_err("UVec3 division by zero"));
        }
        let value = self.as_ref()?;
        Ok(UVec3::new(
            checked(value.x.checked_div(scalar))?,
            checked(value.y.checked_div(scalar))?,
            checked(value.z.checked_div(scalar))?,
        )
        .into())
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
        let value = self.as_ref()?;
        let other = other.as_ref()?;
        let x = checked(value.x.checked_mul(other.x))?;
        let y = checked(value.y.checked_mul(other.y))?;
        let z = checked(value.z.checked_mul(other.z))?;
        checked(checked(x.checked_add(y))?.checked_add(z))
    }

    pub fn length_squared(&self) -> PyResult<u32> {
        let value = self.as_ref()?;
        let x = checked(value.x.checked_mul(value.x))?;
        let y = checked(value.y.checked_mul(value.y))?;
        let z = checked(value.z.checked_mul(value.z))?;
        checked(checked(x.checked_add(y))?.checked_add(z))
    }

    pub fn min_element(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.min_element())
    }

    pub fn max_element(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.max_element())
    }

    pub fn element_sum(&self) -> PyResult<u32> {
        let value = self.as_ref()?;
        checked(checked(value.x.checked_add(value.y))?.checked_add(value.z))
    }

    pub fn element_product(&self) -> PyResult<u32> {
        let value = self.as_ref()?;
        checked(checked(value.x.checked_mul(value.y))?.checked_mul(value.z))
    }

    pub fn with_x(&self, x: u32) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(self.as_ref()?.with_x(x)))
    }

    pub fn with_y(&self, y: u32) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(self.as_ref()?.with_y(y)))
    }

    pub fn with_z(&self, z: u32) -> PyResult<PyUVec3> {
        Ok(PyUVec3::from_uvec3(self.as_ref()?.with_z(z)))
    }

    pub fn to_array(&self) -> PyResult<(u32, u32, u32)> {
        let value = self.as_ref()?;
        Ok((value.x, value.y, value.z))
    }

    pub fn as_tuple(&self) -> PyResult<(u32, u32, u32)> {
        self.to_array()
    }
}
