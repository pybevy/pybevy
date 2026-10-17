use bevy::math::{Affine3A, Mat3A, Mat4, Quat, Vec3, Vec3A};
use glam::camera::{lh, rh};
use pybevy_core::{
    FromBorrowedStorage, StorageRef, ValueStorage,
    public_error::{UNSUPPORTED_COMPARISON, matrix_decomposition_singular},
    repr::{ReprLiteral, object_repr},
};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{mat3a::PyMat3A, mat4::PyMat4, quat::PyQuat, vec3::PyVec3, vec3a::PyVec3A};
use crate::{
    checked_axis_angle,
    mat4::{look_at_inputs, look_to_inputs},
    require_invertible,
    richcmp::comparison_result,
};

#[pyclass(name = "Affine3A", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAffine3A {
    storage: ValueStorage<Affine3A>,
}

impl TryFrom<PyAffine3A> for Affine3A {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_aff: PyAffine3A) -> PyResult<Self> {
        Ok(py_aff.storage.get()?)
    }
}

impl TryFrom<&PyAffine3A> for Affine3A {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_aff: &PyAffine3A) -> PyResult<Self> {
        Ok(py_aff.storage.get()?)
    }
}

impl From<Affine3A> for PyAffine3A {
    #[inline(always)]
    fn from(aff: Affine3A) -> Self {
        PyAffine3A::from_affine3a(aff)
    }
}

impl FromBorrowedStorage<ValueStorage<Affine3A>> for PyAffine3A {
    fn from_borrowed(storage: ValueStorage<Affine3A>) -> Self {
        PyAffine3A { storage }
    }
}

impl PyAffine3A {
    #[inline(always)]
    pub fn from_affine3a(aff: Affine3A) -> Self {
        PyAffine3A {
            storage: ValueStorage::owned(aff),
        }
    }

    #[inline(always)]
    pub const fn affine3a(aff: Affine3A) -> Self {
        PyAffine3A {
            storage: ValueStorage::owned(aff),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Affine3A>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn try_get(&self) -> PyResult<Affine3A> {
        Ok(self.storage.get()?)
    }
}

#[pymethods]
impl PyAffine3A {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Affine3A::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    fn const_identity() -> Self {
        PyAffine3A::affine3a(Affine3A::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    fn const_zero() -> Self {
        PyAffine3A::affine3a(Affine3A::ZERO)
    }

    #[staticmethod]
    #[pyo3(name = "NAN")]
    fn const_nan() -> Self {
        PyAffine3A::affine3a(Affine3A::NAN)
    }

    #[new]
    #[pyo3(signature = (
        *,
        matrix3=PyMat3A::mat3a(Mat3A::IDENTITY),
        translation=PyVec3A::vec3a(Vec3A::ZERO)
    ))]
    pub fn new(matrix3: PyMat3A, translation: PyVec3A) -> PyResult<Self> {
        Ok(PyAffine3A::from_affine3a(Affine3A {
            matrix3: matrix3.try_into()?,
            translation: translation.try_into()?,
        }))
    }

    #[staticmethod]
    pub fn from_cols(
        x_axis: &PyVec3A,
        y_axis: &PyVec3A,
        z_axis: &PyVec3A,
        w_axis: &PyVec3A,
    ) -> PyResult<Self> {
        Ok(PyAffine3A::from_affine3a(Affine3A::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
            z_axis.try_into()?,
            w_axis.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_translation(translation: &PyVec3) -> PyResult<Self> {
        let t: Vec3 = translation.try_into()?;
        Ok(PyAffine3A::from_affine3a(Affine3A::from_translation(t)))
    }

    #[staticmethod]
    pub fn from_quat(rotation: &PyQuat) -> PyResult<Self> {
        let q: Quat = rotation.try_into()?;
        Ok(PyAffine3A::from_affine3a(Affine3A::from_quat(q)))
    }

    #[staticmethod]
    pub fn from_axis_angle(axis: &PyVec3, angle: f32) -> PyResult<Self> {
        let (axis, angle) = checked_axis_angle::<Self>(axis.try_into()?, angle)?;
        Ok(PyAffine3A::from_affine3a(Affine3A::from_axis_angle(
            axis, angle,
        )))
    }

    #[staticmethod]
    pub fn from_rotation_x(angle: f32) -> Self {
        PyAffine3A::from_affine3a(Affine3A::from_rotation_x(angle))
    }

    #[staticmethod]
    pub fn from_rotation_y(angle: f32) -> Self {
        PyAffine3A::from_affine3a(Affine3A::from_rotation_y(angle))
    }

    #[staticmethod]
    pub fn from_rotation_z(angle: f32) -> Self {
        PyAffine3A::from_affine3a(Affine3A::from_rotation_z(angle))
    }

    #[staticmethod]
    pub fn from_scale(scale: &PyVec3) -> PyResult<Self> {
        let s: Vec3 = scale.try_into()?;
        Ok(PyAffine3A::from_affine3a(Affine3A::from_scale(s)))
    }

    #[staticmethod]
    pub fn from_scale_rotation_translation(
        scale: &PyVec3,
        rotation: &PyQuat,
        translation: &PyVec3,
    ) -> PyResult<Self> {
        let s: Vec3 = scale.try_into()?;
        let r: Quat = rotation.try_into()?;
        let t: Vec3 = translation.try_into()?;
        Ok(PyAffine3A::from_affine3a(
            Affine3A::from_scale_rotation_translation(s, r, t),
        ))
    }

    #[staticmethod]
    pub fn from_rotation_translation(rotation: &PyQuat, translation: &PyVec3) -> PyResult<Self> {
        let r: Quat = rotation.try_into()?;
        let t: Vec3 = translation.try_into()?;
        Ok(PyAffine3A::from_affine3a(
            Affine3A::from_rotation_translation(r, t),
        ))
    }

    #[staticmethod]
    pub fn from_mat4(mat: &PyMat4) -> PyResult<Self> {
        let m: Mat4 = mat.try_into()?;
        Ok(PyAffine3A::from_affine3a(Affine3A::from_mat4(m)))
    }

    #[getter]
    pub fn matrix3(&self) -> PyResult<PyMat3A> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|a| &a.matrix3, |a| &mut a.matrix3)?)
    }

    #[setter]
    pub fn set_matrix3(&mut self, value: &PyMat3A) -> PyResult<()> {
        let matrix3 = value.try_into()?;
        self.storage.update(|affine| affine.matrix3 = matrix3)?;
        Ok(())
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec3A> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|a| &a.translation, |a| &mut a.translation)?)
    }

    #[setter]
    pub fn set_translation(&mut self, value: &PyVec3A) -> PyResult<()> {
        let translation = value.try_into()?;
        self.storage
            .update(|affine| affine.translation = translation)?;
        Ok(())
    }

    pub fn transform_point3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        let v: Vec3 = rhs.try_into()?;
        Ok(self.as_ref()?.transform_point3(v).into())
    }

    pub fn transform_point3a(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.transform_point3a(rhs.try_into()?).into())
    }

    pub fn transform_vector3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        let v: Vec3 = rhs.try_into()?;
        Ok(self.as_ref()?.transform_vector3(v).into())
    }

    pub fn transform_vector3a(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.transform_vector3a(rhs.try_into()?).into())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        let inverse = self.as_ref()?.inverse();
        require_invertible("Affine3A", inverse.is_finite())?;
        Ok(PyAffine3A::from_affine3a(inverse))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    fn __mul__(&self, other: &PyAffine3A) -> PyResult<PyAffine3A> {
        Ok(PyAffine3A::from_affine3a(
            *self.as_ref()? * *other.as_ref()?,
        ))
    }

    fn __repr__(&self) -> PyResult<String> {
        let aff = *self.as_ref()?;
        Ok(format!(
            "Affine3A(matrix3={}, translation={})",
            object_repr(PyMat3A::from(aff.matrix3)),
            aff.translation.repr_literal()
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyRef<'_, PyAffine3A>>() else {
            return Ok(py.NotImplemented());
        };
        let self_aff = *self.as_ref()?;
        let other_aff = *other_value.as_ref()?;
        let result = match op {
            CompareOp::Eq => self_aff == other_aff,
            CompareOp::Ne => self_aff != other_aff,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }

    pub fn to_scale_rotation_translation(&self) -> PyResult<(PyVec3, PyQuat, PyVec3)> {
        let affine = *self.as_ref()?;
        let determinant = affine.matrix3.determinant();
        if !determinant.is_finite() || determinant == 0.0 {
            return Err(PyValueError::new_err(matrix_decomposition_singular(
                "Affine3A",
            )));
        }
        let (scale, rotation, translation) = affine.to_scale_rotation_translation();
        Ok((scale.into(), rotation.into(), translation.into()))
    }

    #[staticmethod]
    pub fn look_to_lh(eye: &PyVec3, dir: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (eye, dir, up) = look_to_inputs(eye, dir, up)?;
        Ok(PyAffine3A::from_affine3a(lh::view::look_to_affine3a(
            eye, dir, up,
        )))
    }

    #[staticmethod]
    pub fn look_to_rh(eye: &PyVec3, dir: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (eye, dir, up) = look_to_inputs(eye, dir, up)?;
        Ok(PyAffine3A::from_affine3a(rh::view::look_to_affine3a(
            eye, dir, up,
        )))
    }

    #[staticmethod]
    pub fn look_at_lh(eye: &PyVec3, center: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (eye, dir, up) = look_at_inputs(eye, center, up)?;
        Ok(PyAffine3A::from_affine3a(lh::view::look_to_affine3a(
            eye, dir, up,
        )))
    }

    #[staticmethod]
    pub fn look_at_rh(eye: &PyVec3, center: &PyVec3, up: &PyVec3) -> PyResult<Self> {
        let (eye, dir, up) = look_at_inputs(eye, center, up)?;
        Ok(PyAffine3A::from_affine3a(rh::view::look_to_affine3a(
            eye, dir, up,
        )))
    }
}
