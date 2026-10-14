use bevy::math::{Mat3, Vec3};
use glam::camera::{lh, rh};
use pybevy_core::{
    FromBorrowedStorage, StorageRef, ValueStorage,
    public_error::{UNSUPPORTED_COMPARISON, affine_transform_last_row, euler_axes_not_normalized},
    repr::ReprLiteral,
};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{quat::PyQuat, vec2::PyVec2, vec3::PyVec3};
use crate::{
    checked_axis_angle,
    mat4::{PyMat4, look_at_inputs, look_to_axes},
    matrix_index,
    quat::PyEulerRot,
    require_invertible,
    richcmp::comparison_result,
};

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
    /// glam's 2D transforms assume an affine matrix but only assert it under `glam_assert`.
    fn affine_rows(&self, function: &str) -> PyResult<Mat3> {
        let matrix = *self.as_ref()?;
        if !matrix.row(2).abs_diff_eq(Vec3::Z, 1e-6) {
            return Err(PyValueError::new_err(affine_transform_last_row(
                "Mat3",
                function,
                "(0, 0, 1)",
            )));
        }
        Ok(matrix)
    }

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
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Mat3::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    fn const_identity() -> Self {
        PyMat3::mat3(Mat3::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    fn const_zero() -> Self {
        PyMat3::mat3(Mat3::ZERO)
    }

    #[staticmethod]
    #[pyo3(name = "NAN")]
    fn const_nan() -> Self {
        PyMat3::mat3(Mat3::NAN)
    }

    #[new]
    #[pyo3(signature = (*, x_axis = PyVec3::X, y_axis = PyVec3::Y, z_axis = PyVec3::Z))]
    pub fn new(x_axis: PyVec3, y_axis: PyVec3, z_axis: PyVec3) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
            z_axis.try_into()?,
        )))
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
        let (axis, angle) = checked_axis_angle::<Self>(axis.try_into()?, angle)?;
        Ok(PyMat3::mat3(Mat3::from_axis_angle(axis, angle)))
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

    pub fn col(&self, index: isize) -> PyResult<PyVec3> {
        let index = matrix_index("Column", index, "Mat3", 3)?;
        Ok(self.as_ref()?.col(index).into())
    }

    pub fn row(&self, index: isize) -> PyResult<PyVec3> {
        let index = matrix_index("Row", index, "Mat3", 3)?;
        Ok(self.as_ref()?.row(index).into())
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
        let inverse = self.as_ref()?.inverse();
        require_invertible("Mat3", inverse.is_finite())?;
        Ok(PyMat3::mat3(inverse))
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
        let matrix = self.affine_rows("transform_point2")?;
        Ok(PyVec2::from_vec2(matrix.transform_point2(rhs.try_into()?)))
    }

    pub fn transform_vector2(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        let matrix = self.affine_rows("transform_vector2")?;
        Ok(PyVec2::from_vec2(matrix.transform_vector2(rhs.try_into()?)))
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
        if let Ok(other_mat) = other.cast::<PyMat3>() {
            let other_mat = Mat3::try_from(&*other_mat.try_borrow()?)?;
            Ok(Py::new(py, PyMat3::mat3(self_mat * other_mat))?.into_any())
        } else if let Ok(vec) = other.cast::<PyVec3>() {
            let vec = Vec3::try_from(&*vec.try_borrow()?)?;
            Ok(Py::new(py, PyVec3::from_vec3(self_mat * vec))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat3::mat3(self_mat * scalar))?.into_any())
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
        Ok(format!(
            "Mat3(x_axis={}, y_axis={}, z_axis={})",
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
        let Ok(other_value) = other.extract::<PyMat3>() else {
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

    #[getter]
    pub fn x_axis(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.x_axis, |m| &mut m.x_axis)?)
    }

    #[setter]
    pub fn set_x_axis(&mut self, value: &PyVec3) -> PyResult<()> {
        let x_axis = value.try_into()?;
        self.storage.update(|mat| mat.x_axis = x_axis)?;
        Ok(())
    }

    #[getter]
    pub fn y_axis(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.y_axis, |m| &mut m.y_axis)?)
    }

    #[setter]
    pub fn set_y_axis(&mut self, value: &PyVec3) -> PyResult<()> {
        let y_axis = value.try_into()?;
        self.storage.update(|mat| mat.y_axis = y_axis)?;
        Ok(())
    }

    #[getter]
    pub fn z_axis(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.z_axis, |m| &mut m.z_axis)?)
    }

    #[setter]
    pub fn set_z_axis(&mut self, value: &PyVec3) -> PyResult<()> {
        let z_axis = value.try_into()?;
        self.storage.update(|mat| mat.z_axis = z_axis)?;
        Ok(())
    }

    #[staticmethod]
    pub fn from_euler(order: PyEulerRot, a: f32, b: f32, c: f32) -> Self {
        PyMat3::mat3(Mat3::from_euler(order.into(), a, b, c))
    }

    pub fn to_euler(&self, order: PyEulerRot) -> PyResult<(f32, f32, f32)> {
        let matrix = *self.as_ref()?;
        if !(matrix.x_axis.is_normalized()
            && matrix.y_axis.is_normalized()
            && matrix.z_axis.is_normalized())
        {
            return Err(PyValueError::new_err(euler_axes_not_normalized("Mat3")));
        }
        Ok(matrix.to_euler(order.into()))
    }

    #[staticmethod]
    pub fn from_mat4(m: &PyMat4) -> PyResult<Self> {
        Ok(PyMat3::mat3(Mat3::from_mat4(m.try_into()?)))
    }

    #[staticmethod]
    pub fn look_to_lh(dir: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (dir, up) = look_to_axes(dir, up)?;
        Ok(PyMat3::mat3(lh::view::look_to_mat3(dir, up)))
    }

    #[staticmethod]
    pub fn look_to_rh(dir: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (dir, up) = look_to_axes(dir, up)?;
        Ok(PyMat3::mat3(rh::view::look_to_mat3(dir, up)))
    }

    #[staticmethod]
    pub fn look_at_lh(eye: &PyVec3, center: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (_, dir, up) = look_at_inputs(eye, center, up)?;
        Ok(PyMat3::mat3(lh::view::look_to_mat3(dir, up)))
    }

    #[staticmethod]
    pub fn look_at_rh(eye: &PyVec3, center: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (_, dir, up) = look_at_inputs(eye, center, up)?;
        Ok(PyMat3::mat3(rh::view::look_to_mat3(dir, up)))
    }
}
