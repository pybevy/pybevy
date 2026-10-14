use bevy::math::{Affine2, Mat2, Mat3};
use pybevy_core::{
    FromBorrowedStorage, StorageMut, StorageRef, ValueStorage,
    public_error::{UNSUPPORTED_COMPARISON, matrix_decomposition_singular},
    repr::{ReprLiteral, object_repr},
};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{mat2::PyMat2, mat3::PyMat3, vec2::PyVec2};
use crate::{require_invertible, richcmp::comparison_result};

#[pyclass(name = "Affine2", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAffine2 {
    storage: ValueStorage<Affine2>,
}

impl TryFrom<PyAffine2> for Affine2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py: PyAffine2) -> PyResult<Self> {
        Ok(py.storage.get()?)
    }
}

impl TryFrom<&PyAffine2> for Affine2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py: &PyAffine2) -> PyResult<Self> {
        Ok(py.storage.get()?)
    }
}

impl From<Affine2> for PyAffine2 {
    #[inline(always)]
    fn from(affine: Affine2) -> Self {
        PyAffine2::from_affine2(affine)
    }
}

impl FromBorrowedStorage<ValueStorage<Affine2>> for PyAffine2 {
    fn from_borrowed(storage: ValueStorage<Affine2>) -> Self {
        PyAffine2 { storage }
    }
}

impl PyAffine2 {
    #[inline(always)]
    pub fn from_affine2(affine: Affine2) -> Self {
        PyAffine2 {
            storage: ValueStorage::owned(affine),
        }
    }

    #[inline(always)]
    pub const fn affine2(affine: Affine2) -> Self {
        PyAffine2 {
            storage: ValueStorage::owned(affine),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Affine2>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Affine2>> {
        Ok(self.storage.as_mut()?)
    }

    #[inline(always)]
    pub fn try_get(&self) -> PyResult<Affine2> {
        Ok(self.storage.get()?)
    }

    pub const IDENTITY: PyAffine2 = PyAffine2::affine2(Affine2::IDENTITY);
    pub const ZERO: PyAffine2 = PyAffine2::affine2(Affine2::ZERO);
    pub const NAN: PyAffine2 = PyAffine2::affine2(Affine2::NAN);
}

#[pymethods]
impl PyAffine2 {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Affine2::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    pub fn identity() -> Self {
        Self::affine2(Affine2::IDENTITY)
    }
    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::affine2(Affine2::ZERO)
    }
    #[staticmethod]
    #[pyo3(name = "NAN")]
    pub fn nan() -> Self {
        Self::affine2(Affine2::NAN)
    }

    #[new]
    #[pyo3(signature = (*, matrix2 = PyMat2::mat2(Mat2::IDENTITY), translation = PyVec2::ZERO))]
    pub fn new(matrix2: PyMat2, translation: PyVec2) -> PyResult<Self> {
        Ok(PyAffine2::from_affine2(Affine2 {
            matrix2: matrix2.try_into()?,
            translation: translation.try_into()?,
        }))
    }

    #[staticmethod]
    pub fn from_cols(x_axis: PyVec2, y_axis: PyVec2, z_axis: PyVec2) -> PyResult<Self> {
        Ok(PyAffine2::from_affine2(Affine2::from_cols(
            x_axis.try_into()?,
            y_axis.try_into()?,
            z_axis.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_scale(scale: PyVec2) -> PyResult<Self> {
        Ok(PyAffine2::from_affine2(Affine2::from_scale(
            scale.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_angle(angle: f32) -> Self {
        PyAffine2::from_affine2(Affine2::from_angle(angle))
    }

    #[staticmethod]
    pub fn from_translation(translation: PyVec2) -> PyResult<Self> {
        Ok(PyAffine2::from_affine2(Affine2::from_translation(
            translation.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_scale_angle_translation(
        scale: PyVec2,
        angle: f32,
        translation: PyVec2,
    ) -> PyResult<Self> {
        Ok(PyAffine2::from_affine2(
            Affine2::from_scale_angle_translation(
                scale.try_into()?,
                angle,
                translation.try_into()?,
            ),
        ))
    }

    #[staticmethod]
    pub fn from_mat2(matrix2: PyMat2) -> PyResult<Self> {
        Ok(PyAffine2::from_affine2(Affine2::from_mat2(
            matrix2.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_mat2_translation(matrix2: PyMat2, translation: PyVec2) -> PyResult<Self> {
        Ok(PyAffine2::from_affine2(Affine2::from_mat2_translation(
            matrix2.try_into()?,
            translation.try_into()?,
        )))
    }

    pub fn to_scale_angle_translation(&self) -> PyResult<(PyVec2, f32, PyVec2)> {
        let affine = *self.as_ref()?;
        let determinant = affine.matrix2.determinant();
        if !determinant.is_finite() || determinant == 0.0 {
            return Err(PyValueError::new_err(matrix_decomposition_singular(
                "Affine2",
            )));
        }
        let (scale, angle, translation) = affine.to_scale_angle_translation();
        Ok((scale.into(), angle, translation.into()))
    }

    #[getter]
    pub fn matrix2(&self) -> PyResult<PyMat2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|a| &a.matrix2, |a| &mut a.matrix2)?)
    }

    #[setter]
    pub fn set_matrix2(&mut self, value: PyMat2) -> PyResult<()> {
        self.as_mut()?.matrix2 = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|a| &a.translation, |a| &mut a.translation)?)
    }

    #[setter]
    pub fn set_translation(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.translation = value.try_into()?;
        Ok(())
    }

    pub fn inverse(&self) -> PyResult<PyAffine2> {
        let inverse = self.as_ref()?.inverse();
        require_invertible("Affine2", inverse.is_finite())?;
        Ok(PyAffine2::from_affine2(inverse))
    }

    pub fn transform_point2(&self, point: PyVec2) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.transform_point2(point.try_into()?).into())
    }

    pub fn transform_vector2(&self, vector: PyVec2) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.transform_vector2(vector.try_into()?).into())
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn into_mat3(&self) -> PyResult<PyMat3> {
        let affine = self.as_ref()?;
        let mat3: Mat3 = (*affine).into();
        Ok(mat3.into())
    }

    fn __mul__(&self, other: &PyAffine2) -> PyResult<PyAffine2> {
        Ok(PyAffine2::from_affine2(self.try_get()? * other.try_get()?))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let a = self.as_ref()?;
        Ok(format!(
            "Affine2(matrix2={}, translation={})",
            object_repr(PyMat2::from(a.matrix2)),
            a.translation.repr_literal()
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyAffine2>() else {
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
}
