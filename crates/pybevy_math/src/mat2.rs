use bevy::math::{Mat2, Vec2};
use pybevy_core::{
    FromBorrowedStorage, StorageMut, StorageRef, ValueStorage,
    public_error::UNSUPPORTED_COMPARISON, repr::ReprLiteral,
};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use super::vec2::PyVec2;
use crate::{matrix_index, require_invertible, richcmp::comparison_result};

#[pyclass(name = "Mat2", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMat2 {
    storage: ValueStorage<Mat2>,
}

impl TryFrom<PyMat2> for Mat2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py: PyMat2) -> PyResult<Self> {
        Ok(py.storage.get()?)
    }
}

impl TryFrom<&PyMat2> for Mat2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py: &PyMat2) -> PyResult<Self> {
        Ok(py.storage.get()?)
    }
}

impl From<Mat2> for PyMat2 {
    #[inline(always)]
    fn from(mat: Mat2) -> Self {
        PyMat2::from_mat2(mat)
    }
}

impl FromBorrowedStorage<ValueStorage<Mat2>> for PyMat2 {
    fn from_borrowed(storage: ValueStorage<Mat2>) -> Self {
        PyMat2 { storage }
    }
}

impl PyMat2 {
    #[inline(always)]
    pub fn from_mat2(mat: Mat2) -> Self {
        PyMat2 {
            storage: ValueStorage::owned(mat),
        }
    }

    #[inline(always)]
    pub const fn mat2(mat: Mat2) -> Self {
        PyMat2 {
            storage: ValueStorage::owned(mat),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Mat2>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Mat2>> {
        Ok(self.storage.as_mut()?)
    }

    #[inline(always)]
    pub fn try_get(&self) -> PyResult<Mat2> {
        Ok(self.storage.get()?)
    }
}

#[pymethods]
impl PyMat2 {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Mat2::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    fn const_identity() -> Self {
        PyMat2::mat2(Mat2::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    fn const_zero() -> Self {
        PyMat2::mat2(Mat2::ZERO)
    }

    #[staticmethod]
    #[pyo3(name = "NAN")]
    fn const_nan() -> Self {
        PyMat2::mat2(Mat2::NAN)
    }

    #[new]
    #[pyo3(signature = (*, x_axis = PyVec2::X, y_axis = PyVec2::Y))]
    pub fn new(x_axis: PyVec2, y_axis: PyVec2) -> PyResult<Self> {
        Ok(PyMat2::from_mat2(Mat2::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_cols(x_axis: PyVec2, y_axis: PyVec2) -> PyResult<Self> {
        Ok(PyMat2::from_mat2(Mat2::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_cols_array(values: [f32; 4]) -> Self {
        PyMat2::from_mat2(Mat2::from_cols_array(&values))
    }

    #[staticmethod]
    pub fn from_angle(angle: f32) -> Self {
        PyMat2::from_mat2(Mat2::from_angle(angle))
    }

    #[staticmethod]
    pub fn from_scale_angle(scale: PyVec2, angle: f32) -> PyResult<Self> {
        Ok(PyMat2::from_mat2(Mat2::from_scale_angle(
            scale.try_into()?,
            angle,
        )))
    }

    #[staticmethod]
    pub fn from_diagonal(diagonal: PyVec2) -> PyResult<Self> {
        Ok(PyMat2::from_mat2(Mat2::from_diagonal(diagonal.try_into()?)))
    }

    pub fn col(&self, index: isize) -> PyResult<PyVec2> {
        let index = matrix_index("Column", index, "Mat2", 2)?;
        Ok(self.as_ref()?.col(index).into())
    }

    pub fn row(&self, index: isize) -> PyResult<PyVec2> {
        let index = matrix_index("Row", index, "Mat2", 2)?;
        Ok(self.as_ref()?.row(index).into())
    }

    #[getter]
    pub fn x_axis(&self) -> PyResult<PyVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.x_axis, |m| &mut m.x_axis)?)
    }

    #[setter]
    pub fn set_x_axis(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.x_axis = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn y_axis(&self) -> PyResult<PyVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.y_axis, |m| &mut m.y_axis)?)
    }

    #[setter]
    pub fn set_y_axis(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.y_axis = value.try_into()?;
        Ok(())
    }

    pub fn transpose(&self) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.as_ref()?.transpose()))
    }

    pub fn determinant(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.determinant())
    }

    pub fn inverse(&self) -> PyResult<PyMat2> {
        let inverse = self.as_ref()?.inverse();
        require_invertible("Mat2", inverse.is_finite())?;
        Ok(PyMat2::from_mat2(inverse))
    }

    pub fn mul_vec2(&self, rhs: PyVec2) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.mul_vec2(rhs.try_into()?).into())
    }

    pub fn mul_mat2(&self, rhs: &PyMat2) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(
            self.as_ref()?.mul_mat2(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn mul_scalar(&self, rhs: f32) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(*self.as_ref()? * rhs))
    }

    pub fn abs(&self) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.as_ref()?.abs()))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Ok(other_mat) = other.cast::<PyMat2>() {
            let other_mat = Mat2::try_from(&*other_mat.try_borrow()?)?;
            Ok(Py::new(py, PyMat2::from_mat2(self.try_get()? * other_mat))?.into_any())
        } else if let Ok(other_vec) = other.cast::<PyVec2>() {
            let other_vec = Vec2::try_from(&*other_vec.try_borrow()?)?;
            let transformed: Vec2 = self.try_get()? * other_vec;
            Ok(Py::new(py, PyVec2::from_vec2(transformed))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat2::from_mat2(self.try_get()? * scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __rmul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.try_get()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_mat2(scalar * value))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __add__(&self, other: &PyMat2) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.try_get()? + other.try_get()?))
    }

    fn __sub__(&self, other: &PyMat2) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.try_get()? - other.try_get()?))
    }

    fn __neg__(&self) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(-self.try_get()?))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let m = self.as_ref()?;
        Ok(format!(
            "Mat2(x_axis={}, y_axis={})",
            m.x_axis.repr_literal(),
            m.y_axis.repr_literal()
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyMat2>() else {
            return Ok(py.NotImplemented());
        };
        let a = self.try_get()?;
        let b = other_value.try_get()?;
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }

    pub fn to_cols_array(&self) -> PyResult<[f32; 4]> {
        Ok(self.as_ref()?.to_cols_array())
    }

    #[staticmethod]
    pub fn from_cols_array_2d(m: [[f32; 2]; 2]) -> Self {
        PyMat2::from_mat2(Mat2::from_cols_array_2d(&m))
    }

    pub fn to_cols_array_2d(&self) -> PyResult<[[f32; 2]; 2]> {
        Ok(self.as_ref()?.to_cols_array_2d())
    }

    pub fn add_mat2(&self, rhs: &PyMat2) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(
            self.as_ref()?.add_mat2(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn sub_mat2(&self, rhs: &PyMat2) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(
            self.as_ref()?.sub_mat2(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn div_scalar(&self, rhs: f32) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.as_ref()?.div_scalar(rhs)))
    }
}
