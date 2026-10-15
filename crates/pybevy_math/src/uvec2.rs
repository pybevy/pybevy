use bevy::math::UVec2;
use pybevy_core::{
    FromBorrowedStorage, StorageMut, StorageRef, ValueStorage,
    public_error::{
        CLAMP_MIN_EXCEEDS_MAX, UNSUPPORTED_COMPARISON, integer_vector_division_by_zero,
    },
};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError, PyZeroDivisionError},
    prelude::*,
    types::PyInt,
};

use crate::{
    integer::checked, ivec2::PyIVec2, richcmp::comparison_result, uvec3::PyUVec3, vec2::PyVec2,
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
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(UVec2::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (x=0, y=0))]
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

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyUVec2>() else {
            return Ok(py.NotImplemented());
        };
        let a = self.as_ref()?;
        let b = other_value.as_ref()?;
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }

    pub fn __add__(&self, other: &PyUVec2) -> PyResult<PyUVec2> {
        let value = self.as_ref()?;
        let other = other.as_ref()?;
        Ok(UVec2::new(
            checked(value.x.checked_add(other.x))?,
            checked(value.y.checked_add(other.y))?,
        )
        .into())
    }

    pub fn __sub__(&self, other: &PyUVec2) -> PyResult<PyUVec2> {
        let value = self.as_ref()?;
        let other = other.as_ref()?;
        Ok(UVec2::new(
            checked(value.x.checked_sub(other.x))?,
            checked(value.y.checked_sub(other.y))?,
        )
        .into())
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
        let value = self.as_ref()?;
        let other = other.as_ref()?;
        let x = checked(value.x.checked_mul(other.x))?;
        let y = checked(value.y.checked_mul(other.y))?;
        checked(x.checked_add(y))
    }

    pub fn length_squared(&self) -> PyResult<u32> {
        let value = self.as_ref()?;
        let x = checked(value.x.checked_mul(value.x))?;
        let y = checked(value.y.checked_mul(value.y))?;
        checked(x.checked_add(y))
    }

    pub fn min_element(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.min_element())
    }

    pub fn max_element(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.max_element())
    }

    pub fn element_sum(&self) -> PyResult<u32> {
        let value = self.as_ref()?;
        checked(value.x.checked_add(value.y))
    }

    pub fn element_product(&self) -> PyResult<u32> {
        let value = self.as_ref()?;
        checked(value.x.checked_mul(value.y))
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

    pub fn as_vec2(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.as_vec2().into())
    }

    pub fn as_ivec2(&self) -> PyResult<PyIVec2> {
        Ok(self.as_ref()?.as_ivec2().into())
    }

    pub fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let Some(rhs) = integer_operand(other)? else {
            return Ok(py.NotImplemented());
        };
        let value = *self.as_ref()?;
        let product = UVec2::new(
            checked(value.x.checked_mul(rhs.x))?,
            checked(value.y.checked_mul(rhs.y))?,
        );
        Ok(Py::new(py, PyUVec2::from(product))?.into_any())
    }

    pub fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        self.__mul__(other)
    }

    pub fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let Some(rhs) = integer_operand(other)? else {
            return Ok(py.NotImplemented());
        };
        if rhs.cmpeq(UVec2::ZERO).any() {
            return Err(PyZeroDivisionError::new_err(
                integer_vector_division_by_zero("UVec2"),
            ));
        }
        let value = *self.as_ref()?;
        let quotient = UVec2::new(
            checked(value.x.checked_div(rhs.x))?,
            checked(value.y.checked_div(rhs.y))?,
        );
        Ok(Py::new(py, PyUVec2::from(quotient))?.into_any())
    }

    #[staticmethod]
    #[pyo3(name = "AXES")]
    pub fn axes() -> (PyUVec2, PyUVec2) {
        (Self::from(UVec2::X), Self::from(UVec2::Y))
    }

    pub fn clamp(&self, min: &PyUVec2, max: &PyUVec2) -> PyResult<PyUVec2> {
        let (min, max) = (*min.as_ref()?, *max.as_ref()?);
        if !min.cmple(max).all() {
            return Err(PyValueError::new_err(CLAMP_MIN_EXCEEDS_MAX));
        }
        Ok(self.as_ref()?.clamp(min, max).into())
    }

    pub fn cmpeq(&self, rhs: &PyUVec2) -> PyResult<(bool, bool)> {
        let mask = self.as_ref()?.cmpeq(*rhs.as_ref()?);
        Ok((mask.x, mask.y))
    }

    pub fn cmpne(&self, rhs: &PyUVec2) -> PyResult<(bool, bool)> {
        let mask = self.as_ref()?.cmpne(*rhs.as_ref()?);
        Ok((mask.x, mask.y))
    }

    pub fn cmpge(&self, rhs: &PyUVec2) -> PyResult<(bool, bool)> {
        let mask = self.as_ref()?.cmpge(*rhs.as_ref()?);
        Ok((mask.x, mask.y))
    }

    pub fn cmpgt(&self, rhs: &PyUVec2) -> PyResult<(bool, bool)> {
        let mask = self.as_ref()?.cmpgt(*rhs.as_ref()?);
        Ok((mask.x, mask.y))
    }

    pub fn cmple(&self, rhs: &PyUVec2) -> PyResult<(bool, bool)> {
        let mask = self.as_ref()?.cmple(*rhs.as_ref()?);
        Ok((mask.x, mask.y))
    }

    pub fn cmplt(&self, rhs: &PyUVec2) -> PyResult<(bool, bool)> {
        let mask = self.as_ref()?.cmplt(*rhs.as_ref()?);
        Ok((mask.x, mask.y))
    }

    #[staticmethod]
    pub fn from_array(a: (u32, u32)) -> PyUVec2 {
        UVec2::new(a.0, a.1).into()
    }

    pub fn manhattan_distance(&self, rhs: &PyUVec2) -> PyResult<u32> {
        checked(self.as_ref()?.checked_manhattan_distance(*rhs.as_ref()?))
    }

    pub fn extend(&self, z: u32) -> PyResult<PyUVec3> {
        Ok(self.as_ref()?.extend(z).into())
    }
}

fn integer_operand(other: &Bound<'_, PyAny>) -> PyResult<Option<UVec2>> {
    if let Ok(vector) = other.cast::<PyUVec2>() {
        return Ok(Some(UVec2::try_from(&*vector.try_borrow()?)?));
    }
    if other.is_instance_of::<PyInt>() {
        return Ok(Some(UVec2::splat(other.extract::<u32>()?)));
    }
    Ok(None)
}
