use bevy::math::{Mat3A, Vec3, Vec3A};
use pybevy_core::{
    FromBorrowedStorage, StorageMut, StorageRef, ValueStorage,
    public_error::UNSUPPORTED_COMPARISON, repr::ReprLiteral,
};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use super::{vec3::PyVec3, vec3a::PyVec3A};
use crate::{matrix_index, require_invertible, richcmp::comparison_result};

#[pyclass(name = "Mat3A", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMat3A {
    storage: ValueStorage<Mat3A>,
}

impl TryFrom<PyMat3A> for Mat3A {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_mat: PyMat3A) -> PyResult<Self> {
        Ok(py_mat.storage.get()?)
    }
}

impl TryFrom<&PyMat3A> for Mat3A {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_mat: &PyMat3A) -> PyResult<Self> {
        Ok(py_mat.storage.get()?)
    }
}

impl From<Mat3A> for PyMat3A {
    #[inline(always)]
    fn from(mat: Mat3A) -> Self {
        PyMat3A::from_mat3a(mat)
    }
}

impl FromBorrowedStorage<ValueStorage<Mat3A>> for PyMat3A {
    fn from_borrowed(storage: ValueStorage<Mat3A>) -> Self {
        PyMat3A { storage }
    }
}

impl PyMat3A {
    #[inline(always)]
    pub fn from_mat3a(mat: Mat3A) -> Self {
        PyMat3A {
            storage: ValueStorage::owned(mat),
        }
    }

    #[inline(always)]
    pub const fn mat3a(mat: Mat3A) -> Self {
        PyMat3A {
            storage: ValueStorage::owned(mat),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Mat3A>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Mat3A>> {
        Ok(self.storage.as_mut()?)
    }

    #[inline(always)]
    pub fn try_get(&self) -> PyResult<Mat3A> {
        Ok(self.storage.get()?)
    }
}

#[pymethods]
impl PyMat3A {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Mat3A::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    fn const_identity() -> Self {
        PyMat3A::mat3a(Mat3A::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    fn const_zero() -> Self {
        PyMat3A::mat3a(Mat3A::ZERO)
    }

    #[staticmethod]
    #[pyo3(name = "NAN")]
    fn const_nan() -> Self {
        PyMat3A::mat3a(Mat3A::NAN)
    }

    #[new]
    #[pyo3(signature = (*, x_axis = PyVec3A::vec3a(Vec3A::X), y_axis = PyVec3A::vec3a(Vec3A::Y), z_axis = PyVec3A::vec3a(Vec3A::Z)))]
    pub fn new(x_axis: PyVec3A, y_axis: PyVec3A, z_axis: PyVec3A) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(Mat3A::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
            z_axis.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_cols(x_axis: &PyVec3A, y_axis: &PyVec3A, z_axis: &PyVec3A) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(Mat3A::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
            z_axis.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_cols_array(m: [f32; 9]) -> Self {
        PyMat3A::mat3a(Mat3A::from_cols_array(&m))
    }

    #[staticmethod]
    pub fn from_diagonal(diagonal: &PyVec3) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(Mat3A::from_diagonal(diagonal.try_get()?)))
    }

    #[staticmethod]
    pub fn from_rotation_x(angle: f32) -> Self {
        PyMat3A::mat3a(Mat3A::from_rotation_x(angle))
    }

    #[staticmethod]
    pub fn from_rotation_y(angle: f32) -> Self {
        PyMat3A::mat3a(Mat3A::from_rotation_y(angle))
    }

    #[staticmethod]
    pub fn from_rotation_z(angle: f32) -> Self {
        PyMat3A::mat3a(Mat3A::from_rotation_z(angle))
    }

    #[getter]
    pub fn x_axis(&self) -> PyResult<PyVec3A> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.x_axis, |m| &mut m.x_axis)?)
    }

    #[setter]
    pub fn set_x_axis(&mut self, value: &PyVec3A) -> PyResult<()> {
        self.as_mut()?.x_axis = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn y_axis(&self) -> PyResult<PyVec3A> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.y_axis, |m| &mut m.y_axis)?)
    }

    #[setter]
    pub fn set_y_axis(&mut self, value: &PyVec3A) -> PyResult<()> {
        self.as_mut()?.y_axis = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn z_axis(&self) -> PyResult<PyVec3A> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.z_axis, |m| &mut m.z_axis)?)
    }

    #[setter]
    pub fn set_z_axis(&mut self, value: &PyVec3A) -> PyResult<()> {
        self.as_mut()?.z_axis = value.try_into()?;
        Ok(())
    }

    pub fn col(&self, index: isize) -> PyResult<PyVec3A> {
        let index = matrix_index("Column", index, "Mat3A", 3)?;
        Ok(self.as_ref()?.col(index).into())
    }

    pub fn row(&self, index: isize) -> PyResult<PyVec3A> {
        let index = matrix_index("Row", index, "Mat3A", 3)?;
        Ok(self.as_ref()?.row(index).into())
    }

    pub fn transpose(&self) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(self.as_ref()?.transpose()))
    }

    pub fn determinant(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.determinant())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        let inverse = self.as_ref()?.inverse();
        require_invertible("Mat3A", inverse.is_finite())?;
        Ok(PyMat3A::mat3a(inverse))
    }

    pub fn mul_vec3a(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.mul_vec3a(rhs.try_into()?).into())
    }

    pub fn mul_mat3(&self, rhs: &PyMat3A) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(
            self.as_ref()?.mul_mat3(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn mul_scalar(&self, rhs: f32) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(*self.as_ref()? * rhs))
    }

    pub fn to_cols_array(&self) -> PyResult<[f32; 9]> {
        Ok(self.as_ref()?.to_cols_array())
    }

    pub fn abs(&self) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(self.as_ref()?.abs()))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let self_mat = *self.as_ref()?;
        if let Ok(other_mat) = other.cast::<PyMat3A>() {
            let other_mat = Mat3A::try_from(&*other_mat.try_borrow()?)?;
            Ok(Py::new(py, PyMat3A::mat3a(self_mat * other_mat))?.into_any())
        } else if let Ok(vec) = other.cast::<PyVec3A>() {
            let vec = Vec3A::try_from(&*vec.try_borrow()?)?;
            Ok(Py::new(py, PyVec3A::from_vec3a(self_mat * vec))?.into_any())
        } else if let Ok(vec) = other.cast::<PyVec3>() {
            let vec = Vec3::try_from(&*vec.try_borrow()?)?;
            Ok(Py::new(py, PyVec3::from_vec3(self_mat * vec))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat3A::mat3a(self_mat * scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __rmul__(&self, scalar: f32) -> PyResult<PyMat3A> {
        Ok(PyMat3A::mat3a(scalar * *self.as_ref()?))
    }

    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        let py = other.py();
        if let Ok(other_mat) = other.cast::<PyMat3A>() {
            let other_mat = Mat3A::try_from(&*other_mat.try_borrow()?)?;
            Ok(Py::new(py, Self::mat3a(*value + other_mat))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        let py = other.py();
        if let Ok(other_mat) = other.cast::<PyMat3A>() {
            let other_mat = Mat3A::try_from(&*other_mat.try_borrow()?)?;
            Ok(Py::new(py, Self::mat3a(*value - other_mat))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        let py = other.py();
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::mat3a(*value / scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __rtruediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        let py = other.py();
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::mat3a(scalar / *value))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __neg__(&self) -> PyResult<PyMat3A> {
        Ok(PyMat3A::mat3a(-*self.as_ref()?))
    }

    fn __repr__(&self) -> PyResult<String> {
        let mat = *self.as_ref()?;
        Ok(format!(
            "Mat3A(x_axis={}, y_axis={}, z_axis={})",
            mat.x_axis.repr_literal(),
            mat.y_axis.repr_literal(),
            mat.z_axis.repr_literal()
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyMat3A>() else {
            return Ok(py.NotImplemented());
        };
        let self_mat = *self.as_ref()?;
        let other_mat = *other_value.as_ref()?;
        let result = match op {
            CompareOp::Eq => self_mat == other_mat,
            CompareOp::Ne => self_mat != other_mat,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }

    #[staticmethod]
    pub fn from_cols_array_2d(m: [[f32; 3]; 3]) -> Self {
        PyMat3A::mat3a(Mat3A::from_cols_array_2d(&m))
    }

    pub fn to_cols_array_2d(&self) -> PyResult<[[f32; 3]; 3]> {
        Ok(self.as_ref()?.to_cols_array_2d())
    }

    pub fn add_mat3(&self, rhs: &PyMat3A) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(
            self.as_ref()?.add_mat3(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn sub_mat3(&self, rhs: &PyMat3A) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(
            self.as_ref()?.sub_mat3(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn div_scalar(&self, rhs: f32) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(self.as_ref()?.div_scalar(rhs)))
    }

    pub fn mul_vec3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.mul_vec3(rhs.try_get()?)))
    }
}
