use bevy::math::{Affine2, Mat2, Vec2};
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use super::{mat3::PyMat3, vec2::PyVec2};

#[pyclass(name = "Affine2", from_py_object)]
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
    #[pyo3(signature = (matrix2 = None, translation = None))]
    pub fn new(matrix2: Option<PyMat2>, translation: Option<PyVec2>) -> PyResult<Self> {
        let m2 = matrix2
            .map(TryInto::try_into)
            .transpose()?
            .unwrap_or(Mat2::IDENTITY);
        let t = translation
            .map(TryInto::try_into)
            .transpose()?
            .unwrap_or(Vec2::ZERO);
        Ok(PyAffine2::from_affine2(Affine2::from_mat2_translation(
            m2, t,
        )))
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
        let (scale, angle, translation) = self.as_ref()?.to_scale_angle_translation();
        Ok((scale.into(), angle, translation.into()))
    }

    #[getter]
    pub fn matrix2(&self) -> PyResult<PyMat2> {
        Ok(self.as_ref()?.matrix2.into())
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
        Ok(PyAffine2::from_affine2(self.as_ref()?.inverse()))
    }

    pub fn transform_point2(&self, point: PyVec2) -> PyResult<PyVec2> {
        Ok(self
            .as_ref()?
            .transform_point2(point.try_into()?)
            .try_into()?)
    }

    pub fn transform_vector2(&self, vector: PyVec2) -> PyResult<PyVec2> {
        Ok(self
            .as_ref()?
            .transform_vector2(vector.try_into()?)
            .try_into()?)
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn into_mat3(&self) -> PyResult<PyMat3> {
        let affine = self.as_ref()?;
        let mat3: bevy::math::Mat3 = (*affine).into();
        Ok(mat3.into())
    }

    fn __mul__(&self, other: &PyAffine2) -> PyResult<PyAffine2> {
        Ok(PyAffine2::from_affine2(self.try_get()? * other.try_get()?))
    }

    fn __repr__(&self) -> PyResult<String> {
        let a = self.as_ref()?;
        Ok(format!(
            "Affine2(matrix2={:?}, translation={:?})",
            a.matrix2, a.translation
        ))
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_affine) = other.extract::<PyAffine2>() {
            match op {
                CompareOp::Eq => Ok(self.try_get()? == other_affine.try_get()?),
                CompareOp::Ne => Ok(self.try_get()? != other_affine.try_get()?),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Affine2 with another Affine2",
            ))
        }
    }
}

#[pyclass(name = "Mat2", from_py_object)]
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
    pub fn try_get(&self) -> PyResult<Mat2> {
        Ok(self.storage.get()?)
    }
}

#[pymethods]
impl PyMat2 {
    #[classattr]
    pub const IDENTITY: PyMat2 = PyMat2::mat2(Mat2::IDENTITY);

    #[classattr]
    pub const ZERO: PyMat2 = PyMat2::mat2(Mat2::ZERO);

    #[classattr]
    pub const NAN: PyMat2 = PyMat2::mat2(Mat2::NAN);

    #[new]
    #[pyo3(signature = (x_axis = None, y_axis = None))]
    pub fn new(x_axis: Option<PyVec2>, y_axis: Option<PyVec2>) -> PyResult<Self> {
        let x = x_axis
            .map(TryInto::try_into)
            .transpose()?
            .unwrap_or(Vec2::X);
        let y = y_axis
            .map(TryInto::try_into)
            .transpose()?
            .unwrap_or(Vec2::Y);
        Ok(PyMat2::from_mat2(Mat2::from_cols(x, y)))
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

    pub fn col(&self, index: usize) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.col(index).into())
    }

    pub fn row(&self, index: usize) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.row(index).into())
    }

    #[getter]
    pub fn x_axis(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.x_axis.into())
    }

    #[getter]
    pub fn y_axis(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.y_axis.into())
    }

    pub fn transpose(&self) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.as_ref()?.transpose()))
    }

    pub fn determinant(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.determinant())
    }

    pub fn inverse(&self) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.as_ref()?.inverse()))
    }

    pub fn mul_vec2(&self, rhs: PyVec2) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.mul_vec2(rhs.try_into()?).try_into()?)
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
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyMat2::from_mat2(self.try_get()? * scalar))?.into_any())
        } else if let Ok(other_mat) = other.extract::<PyMat2>() {
            Ok(Py::new(
                py,
                PyMat2::from_mat2(self.try_get()? * other_mat.try_get()?),
            )?
            .into_any())
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

    fn __repr__(&self) -> PyResult<String> {
        let m = self.as_ref()?;
        Ok(format!(
            "Mat2(x_axis={:?}, y_axis={:?})",
            m.x_axis, m.y_axis
        ))
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_mat) = other.extract::<PyMat2>() {
            match op {
                CompareOp::Eq => Ok(self.try_get()? == other_mat.try_get()?),
                CompareOp::Ne => Ok(self.try_get()? != other_mat.try_get()?),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Mat2 with another Mat2",
            ))
        }
    }
}
