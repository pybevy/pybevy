use bevy::math::{Affine3A, Mat3A, Mat4, Quat, Vec3, Vec3A};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use super::{mat3a::PyMat3A, mat4::PyMat4, quat::PyQuat, vec3::PyVec3, vec3a::PyVec3A};

#[pyclass(name = "Affine3A")]
#[derive(Debug, Clone)]
pub struct PyAffine3A {
    storage: ValueStorage<Affine3A>,
}

impl From<PyAffine3A> for Affine3A {
    #[inline(always)]
    fn from(py_aff: PyAffine3A) -> Self {
        py_aff.storage.get().unwrap()
    }
}

impl From<&PyAffine3A> for Affine3A {
    #[inline(always)]
    fn from(py_aff: &PyAffine3A) -> Self {
        py_aff.storage.get().unwrap()
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
    fn as_ref(&self) -> PyResult<&Affine3A> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn get(&self) -> Affine3A {
        self.storage.get().unwrap()
    }
}

#[pymethods]
impl PyAffine3A {
    #[classattr]
    pub const IDENTITY: PyAffine3A = PyAffine3A::affine3a(Affine3A::IDENTITY);

    #[classattr]
    pub const ZERO: PyAffine3A = PyAffine3A::affine3a(Affine3A::ZERO);

    #[classattr]
    pub const NAN: PyAffine3A = PyAffine3A::affine3a(Affine3A::NAN);

    #[new]
    #[pyo3(signature = (matrix3=PyMat3A::mat3a(Mat3A::IDENTITY), translation=PyVec3A::vec3a(Vec3A::ZERO)))]
    pub fn new(matrix3: PyMat3A, translation: PyVec3A) -> Self {
        PyAffine3A::from_affine3a(Affine3A {
            matrix3: matrix3.into(),
            translation: translation.into(),
        })
    }

    #[staticmethod]
    pub fn from_cols(
        x_axis: &PyVec3A,
        y_axis: &PyVec3A,
        z_axis: &PyVec3A,
        w_axis: &PyVec3A,
    ) -> Self {
        PyAffine3A::from_affine3a(Affine3A::from_cols(
            x_axis.into(),
            y_axis.into(),
            z_axis.into(),
            w_axis.into(),
        ))
    }

    #[staticmethod]
    pub fn from_translation(translation: &PyVec3) -> Self {
        let t: Vec3 = translation.into();
        PyAffine3A::from_affine3a(Affine3A::from_translation(t))
    }

    #[staticmethod]
    pub fn from_quat(rotation: &PyQuat) -> Self {
        let q: Quat = rotation.into();
        PyAffine3A::from_affine3a(Affine3A::from_quat(q))
    }

    #[staticmethod]
    pub fn from_axis_angle(axis: &PyVec3, angle: f32) -> Self {
        let a: Vec3 = axis.into();
        PyAffine3A::from_affine3a(Affine3A::from_axis_angle(a, angle))
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
    pub fn from_scale(scale: &PyVec3) -> Self {
        let s: Vec3 = scale.into();
        PyAffine3A::from_affine3a(Affine3A::from_scale(s))
    }

    #[staticmethod]
    pub fn from_scale_rotation_translation(
        scale: &PyVec3,
        rotation: &PyQuat,
        translation: &PyVec3,
    ) -> Self {
        let s: Vec3 = scale.into();
        let r: Quat = rotation.into();
        let t: Vec3 = translation.into();
        PyAffine3A::from_affine3a(Affine3A::from_scale_rotation_translation(s, r, t))
    }

    #[staticmethod]
    pub fn from_rotation_translation(rotation: &PyQuat, translation: &PyVec3) -> Self {
        let r: Quat = rotation.into();
        let t: Vec3 = translation.into();
        PyAffine3A::from_affine3a(Affine3A::from_rotation_translation(r, t))
    }

    #[staticmethod]
    pub fn from_mat4(mat: &PyMat4) -> Self {
        let m: Mat4 = mat.into();
        PyAffine3A::from_affine3a(Affine3A::from_mat4(m))
    }

    #[getter]
    pub fn matrix3(&self) -> PyResult<PyMat3A> {
        Ok(self.as_ref()?.matrix3.into())
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.translation.into())
    }

    pub fn transform_point3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        let v: Vec3 = rhs.into();
        Ok(self.as_ref()?.transform_point3(v).into())
    }

    pub fn transform_point3a(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.transform_point3a(rhs.into()).into())
    }

    pub fn transform_vector3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        let v: Vec3 = rhs.into();
        Ok(self.as_ref()?.transform_vector3(v).into())
    }

    pub fn transform_vector3a(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.transform_vector3a(rhs.into()).into())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        Ok(PyAffine3A::from_affine3a(self.as_ref()?.inverse()))
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
            "Affine3A(matrix3={:?}, translation={:?})",
            aff.matrix3, aff.translation
        ))
    }

    fn __richcmp__(&self, other: &PyAffine3A, op: CompareOp) -> PyResult<bool> {
        let self_aff = *self.as_ref()?;
        let other_aff = *other.as_ref()?;
        match op {
            CompareOp::Eq => Ok(self_aff == other_aff),
            CompareOp::Ne => Ok(self_aff != other_aff),
            _ => Err(PyTypeError::new_err("Affine3A only supports == and !=")),
        }
    }
}
