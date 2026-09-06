use bevy::math::{Mat3A, Vec3, Vec3A};
use pybevy_core::{FromBorrowedStorage, StorageRef, ValueStorage};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyIndexError, PyTypeError},
    prelude::*,
};

use super::{vec3::PyVec3, vec3a::PyVec3A};

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
    pub fn try_get(&self) -> PyResult<Mat3A> {
        Ok(self.storage.get()?)
    }
}

#[pymethods]
impl PyMat3A {
    #[classattr]
    pub const IDENTITY: PyMat3A = PyMat3A::mat3a(Mat3A::IDENTITY);

    #[classattr]
    pub const ZERO: PyMat3A = PyMat3A::mat3a(Mat3A::ZERO);

    #[classattr]
    pub const NAN: PyMat3A = PyMat3A::mat3a(Mat3A::NAN);

    #[new]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        m00: f32,
        m01: f32,
        m02: f32,
        m10: f32,
        m11: f32,
        m12: f32,
        m20: f32,
        m21: f32,
        m22: f32,
    ) -> Self {
        PyMat3A::mat3a(Mat3A::from_cols_array(&[
            m00, m01, m02, m10, m11, m12, m20, m21, m22,
        ]))
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
        Ok(self.as_ref()?.x_axis.into())
    }

    #[getter]
    pub fn y_axis(&self) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.y_axis.into())
    }

    #[getter]
    pub fn z_axis(&self) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.z_axis.into())
    }

    pub fn col(&self, index: usize) -> PyResult<PyVec3A> {
        let mat = self.as_ref()?;
        match index {
            0 => Ok(mat.x_axis.into()),
            1 => Ok(mat.y_axis.into()),
            2 => Ok(mat.z_axis.into()),
            _ => Err(PyIndexError::new_err("Column index out of range (0-2)")),
        }
    }

    pub fn transpose(&self) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(self.as_ref()?.transpose()))
    }

    pub fn determinant(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.determinant())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        Ok(PyMat3A::mat3a(self.as_ref()?.inverse()))
    }

    pub fn mul_vec3a(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.mul_vec3a(rhs.try_into()?).try_into()?)
    }

    pub fn mul_mat3a(&self, rhs: &PyMat3A) -> PyResult<Self> {
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
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat3A::mat3a(self_mat * scalar))?.into_any())
        } else if let Ok(other_mat) = other.extract::<PyMat3A>() {
            Ok(Py::new(py, PyMat3A::mat3a(self_mat * *other_mat.as_ref()?))?.into_any())
        } else if let Ok(vec) = other.extract::<PyVec3A>() {
            Ok(Py::new(py, PyVec3A::from_vec3a(self_mat * Vec3A::try_from(&vec)?))?.into_any())
        } else if let Ok(vec) = other.extract::<PyVec3>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_mat * Vec3::try_from(&vec)?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __rmul__(&self, scalar: f32) -> PyResult<PyMat3A> {
        Ok(PyMat3A::mat3a(scalar * *self.as_ref()?))
    }

    fn __neg__(&self) -> PyResult<PyMat3A> {
        Ok(PyMat3A::mat3a(-*self.as_ref()?))
    }

    fn __repr__(&self) -> PyResult<String> {
        let mat = *self.as_ref()?;
        Ok(format!(
            "Mat3A(x_axis={:?}, y_axis={:?}, z_axis={:?})",
            mat.x_axis, mat.y_axis, mat.z_axis
        ))
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_mat) = other.extract::<PyMat3A>() {
            let self_mat = *self.as_ref()?;
            let other_mat = *other_mat.as_ref()?;
            match op {
                CompareOp::Eq => Ok(self_mat == other_mat),
                CompareOp::Ne => Ok(self_mat != other_mat),
                _ => Err(PyTypeError::new_err("Mat3A only supports == and !=")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Mat3A with another Mat3A",
            ))
        }
    }
}
