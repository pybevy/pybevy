use bevy::math::{BVec2, Vec2, Vec2Swizzles};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::vec3::PyVec3;

#[pyclass(name = "Vec2", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyVec2 {
    pub(crate) storage: ValueStorage<Vec2>,
}

impl From<PyVec2> for Vec2 {
    #[inline(always)]
    fn from(py_vec: PyVec2) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<&PyVec2> for Vec2 {
    #[inline(always)]
    fn from(py_vec: &PyVec2) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<Vec2> for PyVec2 {
    #[inline(always)]
    fn from(vec: Vec2) -> Self {
        PyVec2::from_vec2(vec)
    }
}

impl FromBorrowedStorage<ValueStorage<Vec2>> for PyVec2 {
    fn from_borrowed(storage: ValueStorage<Vec2>) -> Self {
        PyVec2 { storage }
    }
}

impl PyVec2 {
    #[inline(always)]
    pub fn from_vec2(vec: Vec2) -> Self {
        PyVec2 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    pub const fn vec2(vec: Vec2) -> Self {
        PyVec2 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Vec2> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Vec2> {
        Ok(self.storage.as_mut()?)
    }

    #[inline(always)]
    pub fn get(&self) -> Vec2 {
        self.storage.get().unwrap()
    }

    pub const ZERO: PyVec2 = PyVec2::vec2(Vec2::ZERO);
    pub const ONE: PyVec2 = PyVec2::vec2(Vec2::ONE);
    pub const NEG_ONE: PyVec2 = PyVec2::vec2(Vec2::NEG_ONE);
    pub const MIN: PyVec2 = PyVec2::vec2(Vec2::MIN);
    pub const MAX: PyVec2 = PyVec2::vec2(Vec2::MAX);
    pub const INFINITY: PyVec2 = PyVec2::vec2(Vec2::INFINITY);
    pub const NEG_INFINITY: PyVec2 = PyVec2::vec2(Vec2::NEG_INFINITY);
    pub const NAN: PyVec2 = PyVec2::vec2(Vec2::NAN);
    pub const X: PyVec2 = PyVec2::vec2(Vec2::X);
    pub const Y: PyVec2 = PyVec2::vec2(Vec2::Y);
    pub const NEG_X: PyVec2 = PyVec2::vec2(Vec2::NEG_X);
    pub const NEG_Y: PyVec2 = PyVec2::vec2(Vec2::NEG_Y);
}

#[pymethods]
impl PyVec2 {
    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::vec2(Vec2::ZERO)
    }
    #[staticmethod]
    #[pyo3(name = "ONE")]
    pub fn one() -> Self {
        Self::vec2(Vec2::ONE)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_ONE")]
    pub fn neg_one() -> Self {
        Self::vec2(Vec2::NEG_ONE)
    }
    #[staticmethod]
    #[pyo3(name = "MIN")]
    pub fn min_value() -> Self {
        Self::vec2(Vec2::MIN)
    }
    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max_value() -> Self {
        Self::vec2(Vec2::MAX)
    }
    #[staticmethod]
    #[pyo3(name = "INFINITY")]
    pub fn infinity() -> Self {
        Self::vec2(Vec2::INFINITY)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_INFINITY")]
    pub fn neg_infinity() -> Self {
        Self::vec2(Vec2::NEG_INFINITY)
    }
    #[staticmethod]
    #[pyo3(name = "NAN")]
    pub fn nan() -> Self {
        Self::vec2(Vec2::NAN)
    }
    #[staticmethod]
    #[pyo3(name = "X")]
    pub fn unit_x() -> Self {
        Self::vec2(Vec2::X)
    }
    #[staticmethod]
    #[pyo3(name = "Y")]
    pub fn unit_y() -> Self {
        Self::vec2(Vec2::Y)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_X")]
    pub fn neg_x() -> Self {
        Self::vec2(Vec2::NEG_X)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_Y")]
    pub fn neg_y() -> Self {
        Self::vec2(Vec2::NEG_Y)
    }

    #[new]
    pub fn new(x: f32, y: f32) -> Self {
        PyVec2::vec2(Vec2::new(x, y))
    }

    #[staticmethod]
    pub fn splat(value: f32) -> Self {
        PyVec2::vec2(Vec2::splat(value))
    }

    #[getter]
    pub fn x(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, x: f32) -> PyResult<()> {
        self.as_mut()?.x = x;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, y: f32) -> PyResult<()> {
        self.as_mut()?.y = y;
        Ok(())
    }

    pub fn dot(&self, other: &PyVec2) -> PyResult<f32> {
        Ok(self.as_ref()?.dot(other.get()))
    }

    pub fn length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length())
    }

    pub fn length_squared(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_squared())
    }

    pub fn normalize(&self) -> PyResult<Self> {
        Ok(PyVec2::from_vec2(self.as_ref()?.normalize()))
    }

    fn __add__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_v = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec2::from_vec2(*self_v + Vec2::splat(scalar)))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec2>() {
            Ok(Py::new(py, PyVec2::from_vec2(*self_v + other_vec.get()))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __sub__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_v = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec2::from_vec2(*self_v - Vec2::splat(scalar)))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec2>() {
            Ok(Py::new(py, PyVec2::from_vec2(*self_v - other_vec.get()))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_v = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec2::from_vec2(*self_v * scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec2>() {
            Ok(Py::new(py, PyVec2::from_vec2(*self_v * other_vec.get()))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __div__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_v = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec2::from_vec2(*self_v / scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec2>() {
            Ok(Py::new(py, PyVec2::from_vec2(*self_v / other_vec.get()))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __truediv__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.__div__(other, py)
    }

    fn __neg__(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(-*self.as_ref()?))
    }

    fn __rmul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_v = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec2::from_vec2(scalar * *self_v))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec2>() {
            Ok(Py::new(py, PyVec2::from_vec2(other_vec.get() * *self_v))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        let v = self.as_ref()?;
        Ok(format!("Vec2({}, {})", v.x, v.y))
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_vec) = other.extract::<PyVec2>() {
            match op {
                CompareOp::Eq => Ok(self.get() == other_vec.get()),
                CompareOp::Ne => Ok(self.get() != other_vec.get()),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else if let Ok(tuple) = other.extract::<(f32, f32)>() {
            match op {
                CompareOp::Eq => Ok(self.as_tuple()? == tuple),
                CompareOp::Ne => Ok(self.as_tuple()? != tuple),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Vec2 with another Vec2 or a tuple of two floats",
            ))
        }
    }

    fn as_tuple(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.x, v.y))
    }

    pub fn is_normalized(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_normalized())
    }

    #[staticmethod]
    pub fn from_array(a: (f32, f32)) -> PyVec2 {
        PyVec2::from_vec2(Vec2::from_array([a.0, a.1]))
    }

    pub fn to_array(&self) -> PyResult<(f32, f32)> {
        let arr = self.as_ref()?.to_array();
        Ok((arr[0], arr[1]))
    }

    pub fn extend(&self, z: f32) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.extend(z).into())
    }

    pub fn abs(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.abs()))
    }

    pub fn signum(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.signum()))
    }

    pub fn copysign(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.copysign(*rhs.as_ref()?)))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn round(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.round()))
    }

    pub fn trunc(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.trunc()))
    }

    pub fn fract(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.fract()))
    }

    pub fn fract_gl(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.fract_gl()))
    }

    pub fn exp(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.exp()))
    }

    pub fn powf(&self, n: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.powf(n)))
    }

    pub fn recip(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.recip()))
    }

    pub fn lerp(&self, rhs: &PyVec2, s: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.lerp(*rhs.as_ref()?, s)))
    }

    pub fn move_towards(&self, rhs: &PyVec2, d: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.move_towards(*rhs.as_ref()?, d),
        ))
    }

    pub fn midpoint(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.midpoint(*rhs.as_ref()?)))
    }

    pub fn project_onto(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.project_onto(*rhs.as_ref()?),
        ))
    }

    pub fn reject_from(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.reject_from(*rhs.as_ref()?),
        ))
    }

    pub fn normalize_or_zero(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.normalize_or_zero()))
    }

    pub fn try_normalize(&self) -> PyResult<Option<PyVec2>> {
        Ok(self.as_ref()?.try_normalize().map(PyVec2::from_vec2))
    }

    pub fn distance(&self, rhs: &PyVec2) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(*rhs.as_ref()?))
    }

    pub fn distance_squared(&self, rhs: &PyVec2) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(*rhs.as_ref()?))
    }

    pub fn min(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.min(*rhs.as_ref()?)))
    }

    pub fn max(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.max(*rhs.as_ref()?)))
    }

    pub fn clamp(&self, min: &PyVec2, max: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.clamp(*min.as_ref()?, *max.as_ref()?),
        ))
    }

    pub fn element_sum(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.element_sum())
    }

    pub fn element_product(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.element_product())
    }

    pub fn min_element(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_element())
    }

    pub fn max_element(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_element())
    }

    pub fn floor(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.floor()))
    }

    pub fn ceil(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.ceil()))
    }

    pub fn with_x(&self, x: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.with_x(x)))
    }

    pub fn with_y(&self, y: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.with_y(y)))
    }

    pub fn length_recip(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_recip())
    }

    pub fn clamp_length(&self, min: f32, max: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.clamp_length(min, max)))
    }

    pub fn clamp_length_min(&self, min: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.clamp_length_min(min)))
    }

    pub fn clamp_length_max(&self, max: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.clamp_length_max(max)))
    }

    pub fn project_onto_normalized(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.project_onto_normalized(*rhs.as_ref()?),
        ))
    }

    pub fn reject_from_normalized(&self, rhs: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.reject_from_normalized(*rhs.as_ref()?),
        ))
    }

    #[staticmethod]
    pub fn select(mask: (bool, bool), if_true: &PyVec2, if_false: &PyVec2) -> PyResult<PyVec2> {
        let bmask = BVec2::new(mask.0, mask.1);
        Ok(PyVec2::from_vec2(Vec2::select(
            bmask,
            *if_true.as_ref()?,
            *if_false.as_ref()?,
        )))
    }

    pub fn cmpeq(&self, rhs: &PyVec2) -> PyResult<(bool, bool)> {
        let result = self.as_ref()?.cmpeq(*rhs.as_ref()?);
        Ok((result.x, result.y))
    }

    pub fn cmpne(&self, rhs: &PyVec2) -> PyResult<(bool, bool)> {
        let result = self.as_ref()?.cmpne(*rhs.as_ref()?);
        Ok((result.x, result.y))
    }

    pub fn cmpge(&self, rhs: &PyVec2) -> PyResult<(bool, bool)> {
        let result = self.as_ref()?.cmpge(*rhs.as_ref()?);
        Ok((result.x, result.y))
    }

    pub fn cmpgt(&self, rhs: &PyVec2) -> PyResult<(bool, bool)> {
        let result = self.as_ref()?.cmpgt(*rhs.as_ref()?);
        Ok((result.x, result.y))
    }

    pub fn cmple(&self, rhs: &PyVec2) -> PyResult<(bool, bool)> {
        let result = self.as_ref()?.cmple(*rhs.as_ref()?);
        Ok((result.x, result.y))
    }

    pub fn cmplt(&self, rhs: &PyVec2) -> PyResult<(bool, bool)> {
        let result = self.as_ref()?.cmplt(*rhs.as_ref()?);
        Ok((result.x, result.y))
    }

    #[staticmethod]
    pub fn from_slice(slice: Vec<f32>) -> PyResult<PyVec2> {
        if slice.len() < 2 {
            return Err(PyValueError::new_err("Slice must have at least 2 elements"));
        }
        Ok(PyVec2::from_vec2(Vec2::from_slice(&slice)))
    }

    pub fn angle_between(&self, other: &PyVec2) -> PyResult<f32> {
        Ok(self.as_ref()?.angle_to(*other.as_ref()?))
    }

    pub fn perp(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.perp()))
    }

    pub fn perp_dot(&self, other: &PyVec2) -> PyResult<f32> {
        Ok(self.as_ref()?.perp_dot(*other.as_ref()?))
    }

    pub fn rotate(&self, other: &PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.rotate(*other.as_ref()?)))
    }

    #[staticmethod]
    pub fn from_angle(angle: f32) -> PyVec2 {
        PyVec2::from_vec2(Vec2::from_angle(angle))
    }

    pub fn to_angle(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.to_angle())
    }

    pub fn xx(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.xx().into())
    }

    pub fn xy(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.xy().into())
    }

    pub fn yx(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.yx().into())
    }

    pub fn yy(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.yy().into())
    }
}
