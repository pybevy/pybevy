use bevy::math::{Mat3, Vec3};
use pybevy_core::{FromBorrowedStorage, StorageRef, ValueStorage};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{quat::PyQuat, vec2::PyVec2, vec3::PyVec3};

#[pyclass(name = "Mat3", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMat3 {
    storage: ValueStorage<Mat3>,
}

impl TryFrom<PyMat3> for Mat3 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_mat: PyMat3) -> PyResult<Self> {
        Ok(py_mat.storage.get()?)
    }
}

impl TryFrom<&PyMat3> for Mat3 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_mat: &PyMat3) -> PyResult<Self> {
        Ok(py_mat.storage.get()?)
    }
}

impl From<Mat3> for PyMat3 {
    #[inline(always)]
    fn from(mat: Mat3) -> Self {
        PyMat3::from_mat3(mat)
    }
}

impl FromBorrowedStorage<ValueStorage<Mat3>> for PyMat3 {
    fn from_borrowed(storage: ValueStorage<Mat3>) -> Self {
        PyMat3 { storage }
    }
}

impl PyMat3 {
    #[inline(always)]
    pub fn from_mat3(mat: Mat3) -> Self {
        PyMat3 {
            storage: ValueStorage::owned(mat),
        }
    }

    #[inline(always)]
    pub fn into_mat3(self) -> PyResult<Mat3> {
        self.try_into()
    }

    #[inline(always)]
    pub const fn mat3(mat: Mat3) -> Self {
        PyMat3 {
            storage: ValueStorage::owned(mat),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Mat3>> {
        Ok(self.storage.as_ref()?)
    }
}

#[pymethods]
impl PyMat3 {
    #[classattr]
    pub const IDENTITY: PyMat3 = PyMat3::mat3(Mat3::IDENTITY);

    #[classattr]
    pub const ZERO: PyMat3 = PyMat3::mat3(Mat3::ZERO);

    #[classattr]
    pub const NAN: PyMat3 = PyMat3::mat3(Mat3::NAN);

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
        PyMat3::mat3(Mat3::from_cols_array(&[
            m00, m01, m02, m10, m11, m12, m20, m21, m22,
        ]))
    }

    #[staticmethod]
    pub fn from_cols(x_axis: &PyVec3, y_axis: &PyVec3, z_axis: &PyVec3) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
            z_axis.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_cols_array(m: [f32; 9]) -> Self {
        PyMat3::mat3(Mat3::from_cols_array(&m))
    }

    #[staticmethod]
    pub fn from_cols_array_2d(m: [[f32; 3]; 3]) -> Self {
        PyMat3::mat3(Mat3::from_cols_array_2d(&m))
    }

    #[staticmethod]
    pub fn from_diagonal(diagonal: &PyVec3) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_diagonal(diagonal.try_into()?)))
    }

    #[staticmethod]
    pub fn from_quat(quat: &PyQuat) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_quat(quat.try_into()?)))
    }

    #[staticmethod]
    pub fn from_axis_angle(axis: &PyVec3, angle: f32) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_axis_angle(axis.try_into()?, angle)))
    }

    #[staticmethod]
    pub fn from_rotation_x(angle: f32) -> Self {
        PyMat3::mat3(Mat3::from_rotation_x(angle))
    }

    #[staticmethod]
    pub fn from_rotation_y(angle: f32) -> Self {
        PyMat3::mat3(Mat3::from_rotation_y(angle))
    }

    #[staticmethod]
    pub fn from_rotation_z(angle: f32) -> Self {
        PyMat3::mat3(Mat3::from_rotation_z(angle))
    }

    #[staticmethod]
    pub fn from_translation(translation: &PyVec2) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_translation(
            translation.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_angle(angle: f32) -> Self {
        PyMat3::mat3(Mat3::from_angle(angle))
    }

    #[staticmethod]
    pub fn from_scale(scale: &PyVec2) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_scale(scale.try_into()?)))
    }

    #[staticmethod]
    pub fn from_scale_angle_translation(
        scale: &PyVec2,
        angle: f32,
        translation: &PyVec2,
    ) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_scale_angle_translation(
            scale.try_into()?,
            angle,
            translation.try_into()?,
        )))
    }

    pub fn col(&self, index: usize) -> PyResult<PyVec3> {
        let mat = self.as_ref()?;
        if index >= 3 {
            return Err(PyValueError::new_err("Column index out of range"));
        }
        Ok(mat.col(index).into())
    }

    pub fn row(&self, index: usize) -> PyResult<PyVec3> {
        let mat = self.as_ref()?;
        if index >= 3 {
            return Err(PyValueError::new_err("Row index out of range"));
        }
        Ok(mat.row(index).into())
    }

    pub fn to_cols_array(&self) -> PyResult<[f32; 9]> {
        Ok(self.as_ref()?.to_cols_array())
    }

    pub fn to_cols_array_2d(&self) -> PyResult<[[f32; 3]; 3]> {
        Ok(self.as_ref()?.to_cols_array_2d())
    }

    pub fn transpose(&self) -> PyResult<Self> {
        Ok(PyMat3::mat3(self.as_ref()?.transpose()))
    }

    pub fn determinant(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.determinant())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        Ok(PyMat3::mat3(self.as_ref()?.inverse()))
    }

    pub fn mul_vec3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.mul_vec3(rhs.try_into()?)))
    }

    pub fn mul_mat3(&self, rhs: &PyMat3) -> PyResult<Self> {
        Ok(PyMat3::mat3(
            self.as_ref()?.mul_mat3(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn add_mat3(&self, rhs: &PyMat3) -> PyResult<Self> {
        Ok(PyMat3::mat3(
            self.as_ref()?.add_mat3(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn sub_mat3(&self, rhs: &PyMat3) -> PyResult<Self> {
        Ok(PyMat3::mat3(
            self.as_ref()?.sub_mat3(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn mul_scalar(&self, rhs: f32) -> PyResult<Self> {
        Ok(PyMat3::mat3(self.as_ref()?.mul_scalar(rhs)))
    }

    pub fn div_scalar(&self, rhs: f32) -> PyResult<Self> {
        Ok(PyMat3::mat3(self.as_ref()?.div_scalar(rhs)))
    }

    pub fn transform_point2(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.transform_point2(rhs.try_into()?),
        ))
    }

    pub fn transform_vector2(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.transform_vector2(rhs.try_into()?),
        ))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn abs(&self) -> PyResult<Self> {
        Ok(PyMat3::mat3(self.as_ref()?.abs()))
    }

    pub fn abs_diff_eq(&self, rhs: &PyMat3, max_abs_diff: f32) -> PyResult<bool> {
        Ok(self.as_ref()?.abs_diff_eq(*rhs.as_ref()?, max_abs_diff))
    }

    fn __add__(&self, other: &PyMat3) -> PyResult<PyMat3> {
        Ok(PyMat3::mat3(*self.as_ref()? + *other.as_ref()?))
    }

    fn __sub__(&self, other: &PyMat3) -> PyResult<PyMat3> {
        Ok(PyMat3::mat3(*self.as_ref()? - *other.as_ref()?))
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let self_mat = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat3::mat3(self_mat * scalar))?.into_any())
        } else if let Ok(other_mat) = other.extract::<PyMat3>() {
            Ok(Py::new(py, PyMat3::mat3(self_mat * *other_mat.as_ref()?))?.into_any())
        } else if let Ok(vec) = other.extract::<PyVec3>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_mat * Vec3::try_from(&vec)?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __rmul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_mat = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat3::mat3(scalar * self_mat))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __truediv__(&self, scalar: f32) -> PyResult<PyMat3> {
        Ok(PyMat3::mat3(*self.as_ref()? / scalar))
    }

    fn __neg__(&self) -> PyResult<PyMat3> {
        Ok(PyMat3::mat3(-*self.as_ref()?))
    }

    fn __repr__(&self) -> PyResult<String> {
        let mat = *self.as_ref()?;
        let arr = mat.to_cols_array();
        Ok(format!(
            "Mat3([{}, {}, {}], [{}, {}, {}], [{}, {}, {}])",
            arr[0], arr[1], arr[2], arr[3], arr[4], arr[5], arr[6], arr[7], arr[8]
        ))
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_mat) = other.extract::<PyMat3>() {
            let self_mat = *self.as_ref()?;
            let other_mat = *other_mat.as_ref()?;
            match op {
                CompareOp::Eq => Ok(self_mat == other_mat),
                CompareOp::Ne => Ok(self_mat != other_mat),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Mat3 with another Mat3",
            ))
        }
    }
}
