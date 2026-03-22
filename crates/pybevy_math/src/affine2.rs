use bevy::math::{Affine2, Mat2, Vec2};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use super::{mat3::PyMat3, vec2::PyVec2};

#[pyclass(name = "Affine2")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAffine2 {
    storage: ValueStorage<Affine2>,
}

impl From<PyAffine2> for Affine2 {
    #[inline(always)]
    fn from(py: PyAffine2) -> Self {
        py.storage.get().unwrap()
    }
}

impl From<&PyAffine2> for Affine2 {
    #[inline(always)]
    fn from(py: &PyAffine2) -> Self {
        py.storage.get().unwrap()
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
    fn as_ref(&self) -> PyResult<&Affine2> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Affine2> {
        Ok(self.storage.as_mut()?)
    }

    #[inline(always)]
    pub fn get(&self) -> Affine2 {
        self.storage.get().unwrap()
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
    pub fn new(matrix2: Option<PyMat2>, translation: Option<PyVec2>) -> Self {
        let m2 = matrix2.map(|m| m.into()).unwrap_or(Mat2::IDENTITY);
        let t = translation.map(|v| v.into()).unwrap_or(Vec2::ZERO);
        PyAffine2::from_affine2(Affine2::from_mat2_translation(m2, t))
    }

    #[staticmethod]
    pub fn from_cols(x_axis: PyVec2, y_axis: PyVec2, z_axis: PyVec2) -> Self {
        PyAffine2::from_affine2(Affine2::from_cols(
            x_axis.into(),
            y_axis.into(),
            z_axis.into(),
        ))
    }

    #[staticmethod]
    pub fn from_scale(scale: PyVec2) -> Self {
        PyAffine2::from_affine2(Affine2::from_scale(scale.into()))
    }

    #[staticmethod]
    pub fn from_angle(angle: f32) -> Self {
        PyAffine2::from_affine2(Affine2::from_angle(angle))
    }

    #[staticmethod]
    pub fn from_translation(translation: PyVec2) -> Self {
        PyAffine2::from_affine2(Affine2::from_translation(translation.into()))
    }

    #[staticmethod]
    pub fn from_scale_angle_translation(scale: PyVec2, angle: f32, translation: PyVec2) -> Self {
        PyAffine2::from_affine2(Affine2::from_scale_angle_translation(
            scale.into(),
            angle,
            translation.into(),
        ))
    }

    #[staticmethod]
    pub fn from_mat2(matrix2: PyMat2) -> Self {
        PyAffine2::from_affine2(Affine2::from_mat2(matrix2.into()))
    }

    #[staticmethod]
    pub fn from_mat2_translation(matrix2: PyMat2, translation: PyVec2) -> Self {
        PyAffine2::from_affine2(Affine2::from_mat2_translation(
            matrix2.into(),
            translation.into(),
        ))
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
        self.as_mut()?.matrix2 = value.into();
        Ok(())
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.translation.into())
    }

    #[setter]
    pub fn set_translation(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.translation = value.into();
        Ok(())
    }

    pub fn inverse(&self) -> PyResult<PyAffine2> {
        Ok(PyAffine2::from_affine2(self.as_ref()?.inverse()))
    }

    pub fn transform_point2(&self, point: PyVec2) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.transform_point2(point.into()).into())
    }

    pub fn transform_vector2(&self, vector: PyVec2) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.transform_vector2(vector.into()).into())
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
        Ok(PyAffine2::from_affine2(self.get() * other.get()))
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
                CompareOp::Eq => Ok(self.get() == other_affine.get()),
                CompareOp::Ne => Ok(self.get() != other_affine.get()),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Affine2 with another Affine2",
            ))
        }
    }
}

#[pyclass(name = "Mat2")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMat2 {
    storage: ValueStorage<Mat2>,
}

impl From<PyMat2> for Mat2 {
    #[inline(always)]
    fn from(py: PyMat2) -> Self {
        py.storage.get().unwrap()
    }
}

impl From<&PyMat2> for Mat2 {
    #[inline(always)]
    fn from(py: &PyMat2) -> Self {
        py.storage.get().unwrap()
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
    fn as_ref(&self) -> PyResult<&Mat2> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn get(&self) -> Mat2 {
        self.storage.get().unwrap()
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
    pub fn new(x_axis: Option<PyVec2>, y_axis: Option<PyVec2>) -> Self {
        let x = x_axis.map(|v| v.into()).unwrap_or(Vec2::X);
        let y = y_axis.map(|v| v.into()).unwrap_or(Vec2::Y);
        PyMat2::from_mat2(Mat2::from_cols(x, y))
    }

    #[staticmethod]
    pub fn from_cols(x_axis: PyVec2, y_axis: PyVec2) -> Self {
        PyMat2::from_mat2(Mat2::from_cols(x_axis.into(), y_axis.into()))
    }

    #[staticmethod]
    pub fn from_angle(angle: f32) -> Self {
        PyMat2::from_mat2(Mat2::from_angle(angle))
    }

    #[staticmethod]
    pub fn from_scale_angle(scale: PyVec2, angle: f32) -> Self {
        PyMat2::from_mat2(Mat2::from_scale_angle(scale.into(), angle))
    }

    #[staticmethod]
    pub fn from_diagonal(diagonal: PyVec2) -> Self {
        PyMat2::from_mat2(Mat2::from_diagonal(diagonal.into()))
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
        Ok(self.as_ref()?.mul_vec2(rhs.into()).into())
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyMat2> {
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(PyMat2::from_mat2(self.get() * scalar))
        } else if let Ok(other_mat) = other.extract::<PyMat2>() {
            Ok(PyMat2::from_mat2(self.get() * other_mat.get()))
        } else {
            Err(PyTypeError::new_err("Unsupported operand type for *"))
        }
    }

    fn __add__(&self, other: &PyMat2) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.get() + other.get()))
    }

    fn __sub__(&self, other: &PyMat2) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(self.get() - other.get()))
    }

    fn __neg__(&self) -> PyResult<PyMat2> {
        Ok(PyMat2::from_mat2(-self.get()))
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
                CompareOp::Eq => Ok(self.get() == other_mat.get()),
                CompareOp::Ne => Ok(self.get() != other_mat.get()),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Mat2 with another Mat2",
            ))
        }
    }
}
