use bevy::math::{Mat4, Vec3, Vec4};
use pybevy_core::{
    FromBorrowedStorage, StorageRef, ValueStorage,
    public_error::{
        DIRECTION_NOT_NORMALIZED, MAT4_LOOK_AT_EYE_EQUALS_CENTER,
        MAT4_LOOK_DIRECTION_PARALLEL_TO_UP, UNSUPPORTED_COMPARISON, UP_NOT_NORMALIZED,
        affine_transform_last_row, euler_axes_not_normalized, mat4_perspective_depth_planes,
        mat4_perspective_near, mat4_projection_not_finite, matrix_decomposition_singular,
    },
    repr::float_repr,
};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{mat3::PyMat3, quat::PyQuat, vec3::PyVec3, vec4::PyVec4};
use crate::{
    matrix_index, quat::PyEulerRot, require_invertible, richcmp::comparison_result, vec3a::PyVec3A,
};

#[pyclass(name = "Mat4", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMat4 {
    storage: ValueStorage<Mat4>,
}

impl TryFrom<PyMat4> for Mat4 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_mat: PyMat4) -> PyResult<Self> {
        Ok(py_mat.storage.get()?)
    }
}

impl TryFrom<&PyMat4> for Mat4 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_mat: &PyMat4) -> PyResult<Self> {
        Ok(py_mat.storage.get()?)
    }
}

impl From<Mat4> for PyMat4 {
    #[inline(always)]
    fn from(mat: Mat4) -> Self {
        PyMat4::from_mat4(mat)
    }
}

impl FromBorrowedStorage<ValueStorage<Mat4>> for PyMat4 {
    fn from_borrowed(storage: ValueStorage<Mat4>) -> Self {
        PyMat4 { storage }
    }
}

impl PyMat4 {
    #[inline(always)]
    pub fn from_mat4(mat: Mat4) -> Self {
        PyMat4 {
            storage: ValueStorage::owned(mat),
        }
    }

    #[inline(always)]
    pub fn into_mat4(self) -> PyResult<Mat4> {
        self.try_into()
    }

    #[inline(always)]
    pub const fn mat4(mat: Mat4) -> Self {
        PyMat4 {
            storage: ValueStorage::owned(mat),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Mat4>> {
        Ok(self.storage.as_ref()?)
    }

    fn affine_rows(&self, function: &str) -> PyResult<Mat4> {
        let matrix = *self.as_ref()?;
        if !matrix.row(3).abs_diff_eq(Vec4::W, 1e-6) {
            return Err(PyValueError::new_err(affine_transform_last_row(
                "Mat4", function,
            )));
        }
        Ok(matrix)
    }
}

fn finite_projection(function: &str, matrix: Mat4) -> PyResult<PyMat4> {
    if !matrix.is_finite() {
        return Err(PyValueError::new_err(mat4_projection_not_finite(function)));
    }
    Ok(PyMat4::mat4(matrix))
}

fn perspective_near(function: &str, z_near: f32) -> PyResult<()> {
    if z_near.is_nan() || z_near <= 0.0 {
        return Err(PyValueError::new_err(mat4_perspective_near(function)));
    }
    Ok(())
}

fn perspective_depth_planes(function: &str, z_near: f32, z_far: f32) -> PyResult<()> {
    if !(z_near > 0.0 && z_far > 0.0) {
        return Err(PyValueError::new_err(mat4_perspective_depth_planes(
            function,
        )));
    }
    Ok(())
}

pub(crate) fn look_direction(eye: Vec3, center: Vec3) -> PyResult<Vec3> {
    (center - eye)
        .try_normalize()
        .ok_or_else(|| PyValueError::new_err(MAT4_LOOK_AT_EYE_EQUALS_CENTER))
}

fn require_not_parallel(dir: Vec3, up: Vec3) -> PyResult<()> {
    if dir.cross(up).try_normalize().is_none() {
        return Err(PyValueError::new_err(MAT4_LOOK_DIRECTION_PARALLEL_TO_UP));
    }
    Ok(())
}

pub(crate) fn look_to_inputs(
    eye: &PyVec3,
    dir: &PyVec3,
    up: &PyVec3,
) -> PyResult<(Vec3, Vec3, Vec3)> {
    let eye: Vec3 = eye.try_into()?;
    let (dir, up) = look_to_axes(dir, up)?;
    Ok((eye, dir, up))
}

pub(crate) fn look_to_axes(dir: &PyVec3, up: &PyVec3) -> PyResult<(Vec3, Vec3)> {
    let dir: Vec3 = dir.try_into()?;
    let up: Vec3 = up.try_into()?;
    if !dir.is_normalized() {
        return Err(PyValueError::new_err(DIRECTION_NOT_NORMALIZED));
    }
    if !up.is_normalized() {
        return Err(PyValueError::new_err(UP_NOT_NORMALIZED));
    }
    require_not_parallel(dir, up)?;
    Ok((dir, up))
}

pub(crate) fn look_at_inputs(
    eye: &PyVec3,
    center: &PyVec3,
    up: &PyVec3,
) -> PyResult<(Vec3, Vec3, Vec3)> {
    let eye: Vec3 = eye.try_into()?;
    let up: Vec3 = up.try_into()?;
    let dir = look_direction(eye, center.try_into()?)?;
    require_not_parallel(dir, up)?;
    Ok((eye, dir, up))
}

#[pymethods]
impl PyMat4 {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Mat4::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    fn const_identity() -> Self {
        PyMat4::mat4(Mat4::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    fn const_zero() -> Self {
        PyMat4::mat4(Mat4::ZERO)
    }

    #[staticmethod]
    #[pyo3(name = "NAN")]
    fn const_nan() -> Self {
        PyMat4::mat4(Mat4::NAN)
    }

    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (m00=1.0, m01=0.0, m02=0.0, m03=0.0, m10=0.0, m11=1.0, m12=0.0, m13=0.0, m20=0.0, m21=0.0, m22=1.0, m23=0.0, m30=0.0, m31=0.0, m32=0.0, m33=1.0))]
    pub fn new(
        m00: f32,
        m01: f32,
        m02: f32,
        m03: f32,
        m10: f32,
        m11: f32,
        m12: f32,
        m13: f32,
        m20: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m30: f32,
        m31: f32,
        m32: f32,
        m33: f32,
    ) -> Self {
        PyMat4::mat4(Mat4::from_cols_array(&[
            m00, m01, m02, m03, m10, m11, m12, m13, m20, m21, m22, m23, m30, m31, m32, m33,
        ]))
    }

    #[staticmethod]
    pub fn from_cols(
        x_axis: &PyVec4,
        y_axis: &PyVec4,
        z_axis: &PyVec4,
        w_axis: &PyVec4,
    ) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
            z_axis.try_into()?,
            w_axis.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_cols_array(m: [f32; 16]) -> Self {
        PyMat4::mat4(Mat4::from_cols_array(&m))
    }

    #[staticmethod]
    pub fn from_cols_array_2d(m: [[f32; 4]; 4]) -> Self {
        PyMat4::mat4(Mat4::from_cols_array_2d(&m))
    }

    #[staticmethod]
    pub fn from_diagonal(diagonal: &PyVec4) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_diagonal(diagonal.try_into()?)))
    }

    #[staticmethod]
    pub fn from_translation(translation: &PyVec3) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_translation(
            translation.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_scale(scale: &PyVec3) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_scale(scale.try_into()?)))
    }

    #[staticmethod]
    pub fn from_quat(quat: &PyQuat) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_quat(quat.try_into()?)))
    }

    #[staticmethod]
    pub fn from_axis_angle(axis: &PyVec3, angle: f32) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_axis_angle(axis.try_into()?, angle)))
    }

    #[staticmethod]
    pub fn from_rotation_x(angle: f32) -> Self {
        PyMat4::mat4(Mat4::from_rotation_x(angle))
    }

    #[staticmethod]
    pub fn from_rotation_y(angle: f32) -> Self {
        PyMat4::mat4(Mat4::from_rotation_y(angle))
    }

    #[staticmethod]
    pub fn from_rotation_z(angle: f32) -> Self {
        PyMat4::mat4(Mat4::from_rotation_z(angle))
    }

    #[staticmethod]
    pub fn from_scale_rotation_translation(
        scale: &PyVec3,
        rotation: &PyQuat,
        translation: &PyVec3,
    ) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_scale_rotation_translation(
            scale.try_into()?,
            rotation.try_into()?,
            translation.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_mat3(mat3: &PyMat3) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_mat3(mat3.try_into()?)))
    }

    #[staticmethod]
    pub fn perspective_lh(
        fov_y_radians: f32,
        aspect_ratio: f32,
        z_near: f32,
        z_far: f32,
    ) -> PyResult<Self> {
        perspective_depth_planes("perspective_lh", z_near, z_far)?;
        finite_projection(
            "perspective_lh",
            Mat4::perspective_lh(fov_y_radians, aspect_ratio, z_near, z_far),
        )
    }

    #[staticmethod]
    pub fn perspective_rh(
        fov_y_radians: f32,
        aspect_ratio: f32,
        z_near: f32,
        z_far: f32,
    ) -> PyResult<Self> {
        perspective_depth_planes("perspective_rh", z_near, z_far)?;
        finite_projection(
            "perspective_rh",
            Mat4::perspective_rh(fov_y_radians, aspect_ratio, z_near, z_far),
        )
    }

    #[staticmethod]
    pub fn perspective_infinite_reverse_rh(
        fov_y_radians: f32,
        aspect_ratio: f32,
        z_near: f32,
    ) -> PyResult<Self> {
        perspective_near("perspective_infinite_reverse_rh", z_near)?;
        finite_projection(
            "perspective_infinite_reverse_rh",
            Mat4::perspective_infinite_reverse_rh(fov_y_radians, aspect_ratio, z_near),
        )
    }

    #[staticmethod]
    pub fn orthographic_lh(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    ) -> PyResult<Self> {
        finite_projection(
            "orthographic_lh",
            Mat4::orthographic_lh(left, right, bottom, top, near, far),
        )
    }

    #[staticmethod]
    pub fn orthographic_rh(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    ) -> PyResult<Self> {
        finite_projection(
            "orthographic_rh",
            Mat4::orthographic_rh(left, right, bottom, top, near, far),
        )
    }

    #[staticmethod]
    pub fn look_at_lh(eye: &PyVec3, center: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (eye, dir, up) = look_at_inputs(eye, center, up)?;
        Ok(PyMat4::mat4(Mat4::look_to_lh(eye, dir, up)))
    }

    #[staticmethod]
    pub fn look_at_rh(eye: &PyVec3, center: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (eye, dir, up) = look_at_inputs(eye, center, up)?;
        Ok(PyMat4::mat4(Mat4::look_to_rh(eye, dir, up)))
    }

    #[staticmethod]
    pub fn look_to_lh(eye: &PyVec3, dir: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (eye, dir, up) = look_to_inputs(eye, dir, up)?;
        Ok(PyMat4::mat4(Mat4::look_to_lh(eye, dir, up)))
    }

    #[staticmethod]
    pub fn look_to_rh(eye: &PyVec3, dir: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (eye, dir, up) = look_to_inputs(eye, dir, up)?;
        Ok(PyMat4::mat4(Mat4::look_to_rh(eye, dir, up)))
    }

    pub fn col(&self, index: isize) -> PyResult<PyVec4> {
        let index = matrix_index("Column", index, "Mat4", 4)?;
        Ok(self.as_ref()?.col(index).into())
    }

    pub fn row(&self, index: isize) -> PyResult<PyVec4> {
        let index = matrix_index("Row", index, "Mat4", 4)?;
        Ok(self.as_ref()?.row(index).into())
    }

    pub fn to_cols_array(&self) -> PyResult<[f32; 16]> {
        Ok(self.as_ref()?.to_cols_array())
    }

    pub fn to_cols_array_2d(&self) -> PyResult<[[f32; 4]; 4]> {
        Ok(self.as_ref()?.to_cols_array_2d())
    }

    pub fn transpose(&self) -> PyResult<Self> {
        Ok(PyMat4::mat4(self.as_ref()?.transpose()))
    }

    pub fn determinant(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.determinant())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        let inverse = self.as_ref()?.inverse();
        require_invertible("Mat4", inverse.is_finite())?;
        Ok(PyMat4::mat4(inverse))
    }

    pub fn mul_vec4(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.mul_vec4(rhs.try_into()?)))
    }

    pub fn mul_mat4(&self, rhs: &PyMat4) -> PyResult<Self> {
        Ok(PyMat4::mat4(
            self.as_ref()?.mul_mat4(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn add_mat4(&self, rhs: &PyMat4) -> PyResult<Self> {
        Ok(PyMat4::mat4(
            self.as_ref()?.add_mat4(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn sub_mat4(&self, rhs: &PyMat4) -> PyResult<Self> {
        Ok(PyMat4::mat4(
            self.as_ref()?.sub_mat4(rhs.as_ref()?.reborrow()),
        ))
    }

    pub fn mul_scalar(&self, rhs: f32) -> PyResult<Self> {
        Ok(PyMat4::mat4(self.as_ref()?.mul_scalar(rhs)))
    }

    pub fn div_scalar(&self, rhs: f32) -> PyResult<Self> {
        Ok(PyMat4::mat4(self.as_ref()?.div_scalar(rhs)))
    }

    pub fn transform_point3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.transform_point3(rhs.try_into()?),
        ))
    }

    pub fn transform_vector3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.transform_vector3(rhs.try_into()?),
        ))
    }

    pub fn project_point3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.project_point3(rhs.try_into()?),
        ))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn abs_diff_eq(&self, rhs: &PyMat4, max_abs_diff: f32) -> PyResult<bool> {
        Ok(self.as_ref()?.abs_diff_eq(*rhs.as_ref()?, max_abs_diff))
    }

    fn __add__(&self, other: &PyMat4) -> PyResult<PyMat4> {
        Ok(PyMat4::mat4(*self.as_ref()? + *other.as_ref()?))
    }

    fn __sub__(&self, other: &PyMat4) -> PyResult<PyMat4> {
        Ok(PyMat4::mat4(*self.as_ref()? - *other.as_ref()?))
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let self_mat = *self.as_ref()?;
        if let Ok(other_mat) = other.cast::<PyMat4>() {
            let other_mat = Mat4::try_from(&*other_mat.try_borrow()?)?;
            Ok(Py::new(py, PyMat4::mat4(self_mat * other_mat))?.into_any())
        } else if let Ok(vec) = other.cast::<PyVec4>() {
            let vec = Vec4::try_from(&*vec.try_borrow()?)?;
            Ok(Py::new(py, PyVec4::from_vec4(self_mat * vec))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat4::mat4(self_mat * scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __rmul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_mat = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat4::mat4(scalar * self_mat))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __truediv__(&self, scalar: f32) -> PyResult<PyMat4> {
        Ok(PyMat4::mat4(*self.as_ref()? / scalar))
    }

    fn __neg__(&self) -> PyResult<PyMat4> {
        Ok(PyMat4::mat4(-*self.as_ref()?))
    }

    fn __repr__(&self) -> PyResult<String> {
        let mat = *self.as_ref()?;
        let arr = mat.to_cols_array().map(float_repr);
        Ok(format!(
            "Mat4([{}, {}, {}, {}], [{}, {}, {}, {}], [{}, {}, {}, {}], [{}, {}, {}, {}])",
            arr[0],
            arr[1],
            arr[2],
            arr[3],
            arr[4],
            arr[5],
            arr[6],
            arr[7],
            arr[8],
            arr[9],
            arr[10],
            arr[11],
            arr[12],
            arr[13],
            arr[14],
            arr[15]
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyMat4>() else {
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
    pub fn x_axis(&self) -> PyResult<PyVec4> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.x_axis, |m| &mut m.x_axis)?)
    }

    #[setter]
    pub fn set_x_axis(&mut self, value: &PyVec4) -> PyResult<()> {
        self.storage.as_mut()?.x_axis = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn y_axis(&self) -> PyResult<PyVec4> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.y_axis, |m| &mut m.y_axis)?)
    }

    #[setter]
    pub fn set_y_axis(&mut self, value: &PyVec4) -> PyResult<()> {
        self.storage.as_mut()?.y_axis = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn z_axis(&self) -> PyResult<PyVec4> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.z_axis, |m| &mut m.z_axis)?)
    }

    #[setter]
    pub fn set_z_axis(&mut self, value: &PyVec4) -> PyResult<()> {
        self.storage.as_mut()?.z_axis = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn w_axis(&self) -> PyResult<PyVec4> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|m| &m.w_axis, |m| &mut m.w_axis)?)
    }

    #[setter]
    pub fn set_w_axis(&mut self, value: &PyVec4) -> PyResult<()> {
        self.storage.as_mut()?.w_axis = value.try_into()?;
        Ok(())
    }

    pub fn to_scale_rotation_translation(&self) -> PyResult<(PyVec3, PyQuat, PyVec3)> {
        let matrix = *self.as_ref()?;
        let determinant = matrix.determinant();
        if !determinant.is_finite() || determinant == 0.0 {
            return Err(PyValueError::new_err(matrix_decomposition_singular("Mat4")));
        }
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Ok((scale.into(), rotation.into(), translation.into()))
    }

    #[staticmethod]
    pub fn from_euler(order: PyEulerRot, a: f32, b: f32, c: f32) -> Self {
        PyMat4::mat4(Mat4::from_euler(order.into(), a, b, c))
    }

    pub fn to_euler(&self, order: PyEulerRot) -> PyResult<(f32, f32, f32)> {
        let matrix = *self.as_ref()?;
        if !(matrix.x_axis.truncate().is_normalized()
            && matrix.y_axis.truncate().is_normalized()
            && matrix.z_axis.truncate().is_normalized())
        {
            return Err(PyValueError::new_err(euler_axes_not_normalized("Mat4")));
        }
        Ok(matrix.to_euler(order.into()))
    }

    pub fn abs(&self) -> PyResult<Self> {
        Ok(PyMat4::mat4(self.as_ref()?.abs()))
    }

    #[staticmethod]
    pub fn from_rotation_translation(rotation: &PyQuat, translation: &PyVec3) -> PyResult<Self> {
        Ok(PyMat4::mat4(Mat4::from_rotation_translation(
            rotation.try_into()?,
            translation.try_into()?,
        )))
    }

    pub fn transform_point3a(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        let matrix = self.affine_rows("transform_point3a")?;
        Ok(matrix.transform_point3a(rhs.try_into()?).into())
    }

    pub fn transform_vector3a(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        let matrix = self.affine_rows("transform_vector3a")?;
        Ok(matrix.transform_vector3a(rhs.try_into()?).into())
    }

    #[staticmethod]
    pub fn perspective_infinite_lh(
        fov_y_radians: f32,
        aspect_ratio: f32,
        z_near: f32,
    ) -> PyResult<Self> {
        perspective_near("perspective_infinite_lh", z_near)?;
        finite_projection(
            "perspective_infinite_lh",
            Mat4::perspective_infinite_lh(fov_y_radians, aspect_ratio, z_near),
        )
    }

    #[staticmethod]
    pub fn perspective_infinite_rh(
        fov_y_radians: f32,
        aspect_ratio: f32,
        z_near: f32,
    ) -> PyResult<Self> {
        perspective_near("perspective_infinite_rh", z_near)?;
        finite_projection(
            "perspective_infinite_rh",
            Mat4::perspective_infinite_rh(fov_y_radians, aspect_ratio, z_near),
        )
    }

    #[staticmethod]
    pub fn perspective_infinite_reverse_lh(
        fov_y_radians: f32,
        aspect_ratio: f32,
        z_near: f32,
    ) -> PyResult<Self> {
        perspective_near("perspective_infinite_reverse_lh", z_near)?;
        finite_projection(
            "perspective_infinite_reverse_lh",
            Mat4::perspective_infinite_reverse_lh(fov_y_radians, aspect_ratio, z_near),
        )
    }
}
