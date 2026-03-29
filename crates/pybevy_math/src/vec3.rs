use bevy::math::{BVec3, Dir3, Vec3, Vec3Swizzles};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{vec2::PyVec2, vec4::PyVec4};

#[pyclass(name = "Vec3")]
#[derive(Debug, Clone)]
pub struct PyVec3 {
    pub(crate) storage: ValueStorage<Vec3>,
}

impl From<PyVec3> for Vec3 {
    #[inline(always)]
    fn from(py_vec: PyVec3) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<&PyVec3> for Vec3 {
    #[inline(always)]
    fn from(py_vec: &PyVec3) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<Vec3> for PyVec3 {
    #[inline(always)]
    fn from(vec: Vec3) -> Self {
        PyVec3::from_vec3(vec)
    }
}

impl TryFrom<PyVec3> for Dir3 {
    type Error = PyErr;

    fn try_from(py_vec: PyVec3) -> Result<Self, Self::Error> {
        Self::try_from(&py_vec)
    }
}

impl TryFrom<&PyVec3> for Dir3 {
    type Error = PyErr;

    fn try_from(py_vec: &PyVec3) -> Result<Self, Self::Error> {
        let vec: Vec3 = py_vec.into();
        Dir3::try_from(vec).map_err(|err| PyValueError::new_err(err.to_string()))
    }
}

impl FromBorrowedStorage<ValueStorage<Vec3>> for PyVec3 {
    fn from_borrowed(storage: ValueStorage<Vec3>) -> Self {
        PyVec3 { storage }
    }
}

impl PyVec3 {
    #[inline(always)]
    pub fn from_vec3(vec: Vec3) -> Self {
        PyVec3 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    pub fn into_vec3(self) -> Vec3 {
        self.into()
    }

    #[inline(always)]
    pub const fn vec3(vec: Vec3) -> Self {
        PyVec3 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Vec3> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Vec3> {
        Ok(self.storage.as_mut()?)
    }

    #[inline(always)]
    pub fn get(&self) -> Vec3 {
        self.storage.get().unwrap()
    }

    pub const ZERO: PyVec3 = PyVec3::vec3(Vec3::ZERO);
    pub const ONE: PyVec3 = PyVec3::vec3(Vec3::ONE);
    pub const NEG_ONE: PyVec3 = PyVec3::vec3(Vec3::NEG_ONE);
    pub const MIN: PyVec3 = PyVec3::vec3(Vec3::MIN);
    pub const MAX: PyVec3 = PyVec3::vec3(Vec3::MAX);
    pub const NAN: PyVec3 = PyVec3::vec3(Vec3::NAN);
    pub const INFINITY: PyVec3 = PyVec3::vec3(Vec3::INFINITY);
    pub const NEG_INFINITY: PyVec3 = PyVec3::vec3(Vec3::NEG_INFINITY);
    pub const X: PyVec3 = PyVec3::vec3(Vec3::X);
    pub const Y: PyVec3 = PyVec3::vec3(Vec3::Y);
    pub const Z: PyVec3 = PyVec3::vec3(Vec3::Z);
    pub const NEG_X: PyVec3 = PyVec3::vec3(Vec3::NEG_X);
    pub const NEG_Y: PyVec3 = PyVec3::vec3(Vec3::NEG_Y);
    pub const NEG_Z: PyVec3 = PyVec3::vec3(Vec3::NEG_Z);
}

#[pymethods]
impl PyVec3 {
    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::vec3(Vec3::ZERO)
    }
    #[staticmethod]
    #[pyo3(name = "ONE")]
    pub fn one() -> Self {
        Self::vec3(Vec3::ONE)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_ONE")]
    pub fn neg_one() -> Self {
        Self::vec3(Vec3::NEG_ONE)
    }
    #[staticmethod]
    #[pyo3(name = "MIN")]
    pub fn min_value() -> Self {
        Self::vec3(Vec3::MIN)
    }
    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max_value() -> Self {
        Self::vec3(Vec3::MAX)
    }
    #[staticmethod]
    #[pyo3(name = "NAN")]
    pub fn nan() -> Self {
        Self::vec3(Vec3::NAN)
    }
    #[staticmethod]
    #[pyo3(name = "INFINITY")]
    pub fn infinity() -> Self {
        Self::vec3(Vec3::INFINITY)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_INFINITY")]
    pub fn neg_infinity() -> Self {
        Self::vec3(Vec3::NEG_INFINITY)
    }
    #[staticmethod]
    #[pyo3(name = "X")]
    pub fn unit_x() -> Self {
        Self::vec3(Vec3::X)
    }
    #[staticmethod]
    #[pyo3(name = "Y")]
    pub fn unit_y() -> Self {
        Self::vec3(Vec3::Y)
    }
    #[staticmethod]
    #[pyo3(name = "Z")]
    pub fn unit_z() -> Self {
        Self::vec3(Vec3::Z)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_X")]
    pub fn neg_x() -> Self {
        Self::vec3(Vec3::NEG_X)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_Y")]
    pub fn neg_y() -> Self {
        Self::vec3(Vec3::NEG_Y)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_Z")]
    pub fn neg_z() -> Self {
        Self::vec3(Vec3::NEG_Z)
    }

    #[new]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        PyVec3::vec3(Vec3::new(x, y, z))
    }

    #[staticmethod]
    pub fn splat(value: f32) -> Self {
        PyVec3::vec3(Vec3::splat(value))
    }

    #[getter]
    pub fn x(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.x = value;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.y = value;
        Ok(())
    }

    #[getter]
    pub fn z(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.z)
    }

    #[setter]
    pub fn set_z(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.z = value;
        Ok(())
    }

    pub fn dot(&self, other: &PyVec3) -> PyResult<f32> {
        Ok(self.as_ref()?.dot(*other.as_ref()?))
    }

    pub fn cross(&self, other: &PyVec3) -> PyResult<Self> {
        Ok(PyVec3::from_vec3(self.as_ref()?.cross(*other.as_ref()?)))
    }

    pub fn length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length())
    }

    pub fn length_squared(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_squared())
    }

    pub fn normalize(&self) -> PyResult<Self> {
        Ok(PyVec3::from_vec3(self.as_ref()?.normalize()))
    }

    pub fn xy(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.xy().into())
    }

    pub fn xz(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.xz().into())
    }

    pub fn yz(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.yz().into())
    }

    pub fn yx(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.yx().into())
    }

    pub fn zx(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.zx().into())
    }

    pub fn zy(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.zy().into())
    }

    pub fn xx(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.xx().into())
    }

    pub fn yy(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.yy().into())
    }

    pub fn zz(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.zz().into())
    }

    pub fn with_x(&self, x: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.with_x(x)))
    }

    pub fn with_y(&self, y: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.with_y(y)))
    }

    pub fn with_z(&self, z: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.with_z(z)))
    }

    pub fn truncate(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.truncate().into())
    }

    pub fn extend(&self, w: f32) -> PyResult<PyVec4> {
        Ok(self.as_ref()?.extend(w).into())
    }

    #[staticmethod]
    pub fn from_array(a: (f32, f32, f32)) -> PyVec3 {
        PyVec3::from_vec3(Vec3::from_array([a.0, a.1, a.2]))
    }

    pub fn to_array(&self) -> PyResult<(f32, f32, f32)> {
        let arr = self.as_ref()?.to_array();
        Ok((arr[0], arr[1], arr[2]))
    }

    pub fn abs(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.abs()))
    }

    pub fn signum(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.signum()))
    }

    pub fn copysign(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.copysign(*rhs.as_ref()?)))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn round(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.round()))
    }

    pub fn trunc(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.trunc()))
    }

    pub fn fract(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.fract()))
    }

    pub fn fract_gl(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.fract_gl()))
    }

    pub fn exp(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.exp()))
    }

    pub fn powf(&self, n: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.powf(n)))
    }

    pub fn recip(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.recip()))
    }

    pub fn lerp(&self, rhs: &PyVec3, s: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.lerp(*rhs.as_ref()?, s)))
    }

    pub fn move_towards(&self, rhs: &PyVec3, d: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.move_towards(*rhs.as_ref()?, d),
        ))
    }

    pub fn midpoint(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.midpoint(*rhs.as_ref()?)))
    }

    pub fn project_onto(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.project_onto(*rhs.as_ref()?),
        ))
    }

    pub fn reject_from(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.reject_from(*rhs.as_ref()?),
        ))
    }

    pub fn normalize_or_zero(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.normalize_or_zero()))
    }

    pub fn try_normalize(&self) -> PyResult<Option<PyVec3>> {
        Ok(self.as_ref()?.try_normalize().map(PyVec3::from_vec3))
    }

    pub fn distance(&self, rhs: &PyVec3) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(*rhs.as_ref()?))
    }

    pub fn distance_squared(&self, rhs: &PyVec3) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(*rhs.as_ref()?))
    }

    pub fn min(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.min(*rhs.as_ref()?)))
    }

    pub fn max(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.max(*rhs.as_ref()?)))
    }

    pub fn clamp(&self, min: &PyVec3, max: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.clamp(*min.as_ref()?, *max.as_ref()?),
        ))
    }

    pub fn element_sum(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.element_sum())
    }

    pub fn element_product(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.element_product())
    }

    fn __add__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_vec + scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec3>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_vec + *other_vec.as_ref()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __sub__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_vec - scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec3>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_vec - *other_vec.as_ref()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_vec * scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec3>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_vec * *other_vec.as_ref()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __div__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_vec / scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec3>() {
            Ok(Py::new(py, PyVec3::from_vec3(self_vec / *other_vec.as_ref()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __truediv__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.__div__(other, py)
    }

    fn __neg__(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(-*self.as_ref()?))
    }

    fn __rmul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(scalar * self_vec))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec3>() {
            Ok(Py::new(py, PyVec3::from_vec3(*other_vec.as_ref()? * self_vec))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        let v = *self.as_ref()?;
        Ok(format!("Vec3({}, {}, {})", v.x, v.y, v.z))
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_vec) = other.extract::<PyVec3>() {
            let self_vec = *self.as_ref()?;
            let other_val = *other_vec.as_ref()?;
            match op {
                CompareOp::Eq => Ok(self_vec == other_val),
                CompareOp::Ne => Ok(self_vec != other_val),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else if let Ok(tuple) = other.extract::<(f32, f32, f32)>() {
            match op {
                CompareOp::Eq => Ok(self.as_tuple()? == tuple),
                CompareOp::Ne => Ok(self.as_tuple()? != tuple),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Vec3 with another Vec3 or a tuple of three floats",
            ))
        }
    }

    fn as_tuple(&self) -> PyResult<(f32, f32, f32)> {
        let v = *self.as_ref()?;
        Ok((v.x, v.y, v.z))
    }

    pub fn is_normalized(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_normalized())
    }

    pub fn floor(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.floor()))
    }

    pub fn ceil(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.ceil()))
    }

    pub fn zxy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zxy()))
    }

    pub fn xyz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xyz()))
    }

    pub fn xzy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xzy()))
    }

    pub fn yxz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yxz()))
    }

    pub fn yzx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yzx()))
    }

    pub fn zyx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zyx()))
    }

    pub fn xxx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xxx()))
    }

    pub fn xxy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xxy()))
    }

    pub fn xxz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xxz()))
    }

    pub fn xyx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xyx()))
    }

    pub fn xyy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xyy()))
    }

    pub fn xzx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xzx()))
    }

    pub fn xzz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.xzz()))
    }

    pub fn yxx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yxx()))
    }

    pub fn yxy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yxy()))
    }

    pub fn yyx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yyx()))
    }

    pub fn yyy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yyy()))
    }

    pub fn yyz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yyz()))
    }

    pub fn yzy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yzy()))
    }

    pub fn yzz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.yzz()))
    }

    pub fn zxx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zxx()))
    }

    pub fn zxz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zxz()))
    }

    pub fn zyy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zyy()))
    }

    pub fn zyz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zyz()))
    }

    pub fn zzx(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zzx()))
    }

    pub fn zzy(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zzy()))
    }

    pub fn zzz(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.zzz()))
    }

    pub fn min_element(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_element())
    }

    pub fn max_element(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_element())
    }

    pub fn angle_between(&self, other: &PyVec3) -> PyResult<f32> {
        Ok(self.as_ref()?.angle_between(*other.as_ref()?))
    }

    pub fn any_orthogonal_vector(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.any_orthogonal_vector().into())
    }

    pub fn any_orthonormal_vector(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.any_orthonormal_vector().into())
    }

    pub fn any_orthonormal_pair(&self) -> PyResult<(PyVec3, PyVec3)> {
        let (v1, v2) = self.as_ref()?.any_orthonormal_pair();
        Ok((PyVec3::from_vec3(v1), PyVec3::from_vec3(v2)))
    }

    pub fn length_recip(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_recip())
    }

    pub fn slerp(&self, rhs: &PyVec3, s: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.slerp(*rhs.as_ref()?, s)))
    }

    pub fn clamp_length(&self, min: f32, max: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.clamp_length(min, max)))
    }

    pub fn clamp_length_min(&self, min: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.clamp_length_min(min)))
    }

    pub fn clamp_length_max(&self, max: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.clamp_length_max(max)))
    }

    pub fn project_onto_normalized(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.project_onto_normalized(*rhs.as_ref()?),
        ))
    }

    pub fn reject_from_normalized(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(
            self.as_ref()?.reject_from_normalized(*rhs.as_ref()?),
        ))
    }

    #[staticmethod]
    pub fn select(
        mask: (bool, bool, bool),
        if_true: &PyVec3,
        if_false: &PyVec3,
    ) -> PyResult<PyVec3> {
        let bmask = BVec3::new(mask.0, mask.1, mask.2);
        Ok(PyVec3::from_vec3(Vec3::select(
            bmask,
            *if_true.as_ref()?,
            *if_false.as_ref()?,
        )))
    }

    pub fn cmpeq(&self, rhs: &PyVec3) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmpeq(*rhs.as_ref()?);
        Ok((result.x, result.y, result.z))
    }

    pub fn cmpne(&self, rhs: &PyVec3) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmpne(*rhs.as_ref()?);
        Ok((result.x, result.y, result.z))
    }

    pub fn cmpge(&self, rhs: &PyVec3) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmpge(*rhs.as_ref()?);
        Ok((result.x, result.y, result.z))
    }

    pub fn cmpgt(&self, rhs: &PyVec3) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmpgt(*rhs.as_ref()?);
        Ok((result.x, result.y, result.z))
    }

    pub fn cmple(&self, rhs: &PyVec3) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmple(*rhs.as_ref()?);
        Ok((result.x, result.y, result.z))
    }

    pub fn cmplt(&self, rhs: &PyVec3) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmplt(*rhs.as_ref()?);
        Ok((result.x, result.y, result.z))
    }

    #[staticmethod]
    pub fn from_slice(slice: Vec<f32>) -> PyResult<PyVec3> {
        if slice.len() < 3 {
            return Err(PyValueError::new_err("Slice must have at least 3 elements"));
        }
        Ok(PyVec3::from_vec3(Vec3::from_slice(&slice)))
    }
}
