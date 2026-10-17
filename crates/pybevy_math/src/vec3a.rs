use bevy::math::{BVec3A, Vec3, Vec3A, Vec3Swizzles};
use pybevy_core::{
    FromBorrowedStorage, StorageRef, ValueStorage,
    domain::{NON_NEGATIVE, check_for},
    public_error::{
        AXIS_NOT_NORMALIZED, CLAMP_MIN_EXCEEDS_MAX, NORMAL_NOT_NORMALIZED, RHS_NOT_NORMALIZED,
        ROTATE_TOWARDS_UNDEFINED_ANGLE, UNSUPPORTED_COMPARISON, VECTOR_NOT_NORMALIZED,
        from_slice_too_short,
    },
    repr::ReprLiteral,
};
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::{richcmp::comparison_result, uvec3::PyUVec3, vec2::PyVec2, vec3::PyVec3, vec4::PyVec4};

#[pyclass(name = "Vec3A", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyVec3A {
    storage: ValueStorage<Vec3A>,
}

impl TryFrom<PyVec3A> for Vec3A {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_vec: PyVec3A) -> PyResult<Self> {
        Ok(py_vec.storage.get()?)
    }
}

impl TryFrom<&PyVec3A> for Vec3A {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_vec: &PyVec3A) -> PyResult<Self> {
        Ok(py_vec.storage.get()?)
    }
}

impl From<Vec3A> for PyVec3A {
    #[inline(always)]
    fn from(vec: Vec3A) -> Self {
        PyVec3A::from_vec3a(vec)
    }
}

impl FromBorrowedStorage<ValueStorage<Vec3A>> for PyVec3A {
    fn from_borrowed(storage: ValueStorage<Vec3A>) -> Self {
        PyVec3A { storage }
    }
}

impl PyVec3A {
    #[inline(always)]
    pub fn from_vec3a(vec: Vec3A) -> Self {
        PyVec3A {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    pub const fn vec3a(vec: Vec3A) -> Self {
        PyVec3A {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Vec3A>> {
        Ok(self.storage.as_ref()?)
    }
}

#[pymethods]
impl PyVec3A {
    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::vec3a(Vec3A::ZERO)
    }

    #[staticmethod]
    #[pyo3(name = "ONE")]
    pub fn one() -> Self {
        Self::vec3a(Vec3A::ONE)
    }

    #[staticmethod]
    #[pyo3(name = "NEG_ONE")]
    pub fn neg_one() -> Self {
        Self::vec3a(Vec3A::NEG_ONE)
    }

    #[staticmethod]
    #[pyo3(name = "MIN")]
    pub fn min_constant() -> Self {
        Self::vec3a(Vec3A::MIN)
    }

    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max_constant() -> Self {
        Self::vec3a(Vec3A::MAX)
    }

    #[staticmethod]
    #[pyo3(name = "NAN")]
    pub fn nan() -> Self {
        Self::vec3a(Vec3A::NAN)
    }

    #[staticmethod]
    #[pyo3(name = "INFINITY")]
    pub fn infinity() -> Self {
        Self::vec3a(Vec3A::INFINITY)
    }

    #[staticmethod]
    #[pyo3(name = "NEG_INFINITY")]
    pub fn neg_infinity() -> Self {
        Self::vec3a(Vec3A::NEG_INFINITY)
    }

    #[staticmethod]
    #[pyo3(name = "X")]
    pub fn unit_x() -> Self {
        Self::vec3a(Vec3A::X)
    }

    #[staticmethod]
    #[pyo3(name = "Y")]
    pub fn unit_y() -> Self {
        Self::vec3a(Vec3A::Y)
    }

    #[staticmethod]
    #[pyo3(name = "Z")]
    pub fn unit_z() -> Self {
        Self::vec3a(Vec3A::Z)
    }

    #[staticmethod]
    #[pyo3(name = "NEG_X")]
    pub fn neg_x() -> Self {
        Self::vec3a(Vec3A::NEG_X)
    }

    #[staticmethod]
    #[pyo3(name = "NEG_Y")]
    pub fn neg_y() -> Self {
        Self::vec3a(Vec3A::NEG_Y)
    }

    #[staticmethod]
    #[pyo3(name = "NEG_Z")]
    pub fn neg_z() -> Self {
        Self::vec3a(Vec3A::NEG_Z)
    }

    #[new]
    #[pyo3(signature = (x=0.0, y=0.0, z=0.0))]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        PyVec3A::vec3a(Vec3A::new(x, y, z))
    }

    #[staticmethod]
    pub fn splat(value: f32) -> Self {
        PyVec3A::vec3a(Vec3A::splat(value))
    }

    #[staticmethod]
    pub fn from_vec3(v: &PyVec3) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(Vec3A::from(v.try_get()?)))
    }

    pub fn to_vec3(&self) -> PyResult<PyVec3> {
        Ok(Vec3::from(*self.as_ref()?).into())
    }

    #[getter]
    pub fn x(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, value: f32) -> PyResult<()> {
        self.storage.update(|vec| vec.x = value)?;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, value: f32) -> PyResult<()> {
        self.storage.update(|vec| vec.y = value)?;
        Ok(())
    }

    #[getter]
    pub fn z(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.z)
    }

    #[setter]
    pub fn set_z(&mut self, value: f32) -> PyResult<()> {
        self.storage.update(|vec| vec.z = value)?;
        Ok(())
    }

    pub fn dot(&self, other: &PyVec3A) -> PyResult<f32> {
        Ok(self.as_ref()?.dot(*other.as_ref()?))
    }

    pub fn cross(&self, other: &PyVec3A) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.cross(*other.as_ref()?)))
    }

    pub fn length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length())
    }

    pub fn length_squared(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_squared())
    }

    pub fn normalize(&self) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.normalize()))
    }

    pub fn normalize_or_zero(&self) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.normalize_or_zero()))
    }

    pub fn abs(&self) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.abs()))
    }

    pub fn min(&self, other: &PyVec3A) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.min(*other.as_ref()?)))
    }

    pub fn max(&self, other: &PyVec3A) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.max(*other.as_ref()?)))
    }

    pub fn distance(&self, other: &PyVec3A) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(*other.as_ref()?))
    }

    pub fn distance_squared(&self, other: &PyVec3A) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(*other.as_ref()?))
    }

    pub fn lerp(&self, other: &PyVec3A, t: f32) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(
            self.as_ref()?.lerp(*other.as_ref()?, t),
        ))
    }

    pub fn __add__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(other_vec) = other.cast::<PyVec3A>() {
            let other_vec = Vec3A::try_from(&*other_vec.try_borrow()?)?;
            Ok(Py::new(py, Self::from_vec3a(*value + other_vec))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_vec3a(*value + scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __sub__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(other_vec) = other.cast::<PyVec3A>() {
            let other_vec = Vec3A::try_from(&*other_vec.try_borrow()?)?;
            Ok(Py::new(py, Self::from_vec3a(*value - other_vec))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_vec3a(*value - scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(other_vec) = other.cast::<PyVec3A>() {
            let other_vec = Vec3A::try_from(&*other_vec.try_borrow()?)?;
            Ok(Py::new(py, Self::from_vec3a(*value * other_vec))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_vec3a(*value * scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __truediv__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(other_vec) = other.cast::<PyVec3A>() {
            let other_vec = Vec3A::try_from(&*other_vec.try_borrow()?)?;
            Ok(Py::new(py, Self::from_vec3a(*value / other_vec))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_vec3a(*value / scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __radd__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_vec3a(scalar + *value))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __rsub__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_vec3a(scalar - *value))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __rmul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_vec3a(scalar * *value))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __rtruediv__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_vec3a(scalar / *value))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __neg__(&self) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(-*self.as_ref()?))
    }

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let other = if let Ok(other_vec) = other.extract::<PyVec3A>() {
            *other_vec.as_ref()?
        } else {
            return Ok(py.NotImplemented());
        };

        let equal = *self.as_ref()? == other;
        let result = match op {
            CompareOp::Eq => equal,
            CompareOp::Ne => !equal,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }

    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(PyVec3A {
            storage: ValueStorage::owned(*self.as_ref()?),
        })
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let v = self.as_ref()?;
        Ok(v.repr_literal())
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

    pub fn with_x(&self, x: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.with_x(x)))
    }

    pub fn with_y(&self, y: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.with_y(y)))
    }

    pub fn with_z(&self, z: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.with_z(z)))
    }

    pub fn truncate(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.truncate().into())
    }

    pub fn extend(&self, w: f32) -> PyResult<PyVec4> {
        Ok(self.as_ref()?.extend(w).into())
    }

    #[staticmethod]
    pub fn from_array(a: (f32, f32, f32)) -> PyVec3A {
        PyVec3A::from_vec3a(Vec3A::from_array([a.0, a.1, a.2]))
    }

    pub fn to_array(&self) -> PyResult<(f32, f32, f32)> {
        let arr = self.as_ref()?.to_array();
        Ok((arr[0], arr[1], arr[2]))
    }

    pub fn signum(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.signum()))
    }

    pub fn copysign(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.copysign(*rhs.as_ref()?)))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn abs_diff_eq(&self, rhs: &PyVec3A, max_abs_diff: f32) -> PyResult<bool> {
        Ok(self.as_ref()?.abs_diff_eq(*rhs.as_ref()?, max_abs_diff))
    }

    pub fn round(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.round()))
    }

    pub fn trunc(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.trunc()))
    }

    pub fn fract(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.fract()))
    }

    pub fn fract_gl(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.fract_gl()))
    }

    pub fn exp(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.exp()))
    }

    pub fn powf(&self, n: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.powf(n)))
    }

    pub fn recip(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.recip()))
    }

    pub fn move_towards(&self, rhs: &PyVec3A, d: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(
            self.as_ref()?.move_towards(*rhs.as_ref()?, d),
        ))
    }

    pub fn midpoint(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.midpoint(*rhs.as_ref()?)))
    }

    pub fn project_onto(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(
            self.as_ref()?.project_onto(*rhs.as_ref()?),
        ))
    }

    pub fn reject_from(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(
            self.as_ref()?.reject_from(*rhs.as_ref()?),
        ))
    }

    pub fn try_normalize(&self) -> PyResult<Option<PyVec3A>> {
        Ok(self.as_ref()?.try_normalize().map(PyVec3A::from_vec3a))
    }

    pub fn clamp(&self, min: &PyVec3A, max: &PyVec3A) -> PyResult<PyVec3A> {
        let (min, max) = (*min.as_ref()?, *max.as_ref()?);
        if !min.cmple(max).all() {
            return Err(PyValueError::new_err(CLAMP_MIN_EXCEEDS_MAX));
        }
        Ok(PyVec3A::from_vec3a(self.as_ref()?.clamp(min, max)))
    }

    pub fn element_sum(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.element_sum())
    }

    pub fn element_product(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.element_product())
    }

    pub fn is_normalized(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_normalized())
    }

    pub fn floor(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.floor()))
    }

    pub fn ceil(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.ceil()))
    }

    pub fn zxy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zxy()))
    }

    pub fn xyz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xyz()))
    }

    pub fn xzy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xzy()))
    }

    pub fn yxz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yxz()))
    }

    pub fn yzx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yzx()))
    }

    pub fn zyx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zyx()))
    }

    pub fn xxx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xxx()))
    }

    pub fn xxy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xxy()))
    }

    pub fn xxz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xxz()))
    }

    pub fn xyx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xyx()))
    }

    pub fn xyy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xyy()))
    }

    pub fn xzx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xzx()))
    }

    pub fn xzz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.xzz()))
    }

    pub fn yxx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yxx()))
    }

    pub fn yxy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yxy()))
    }

    pub fn yyx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yyx()))
    }

    pub fn yyy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yyy()))
    }

    pub fn yyz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yyz()))
    }

    pub fn yzy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yzy()))
    }

    pub fn yzz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.yzz()))
    }

    pub fn zxx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zxx()))
    }

    pub fn zxz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zxz()))
    }

    pub fn zyy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zyy()))
    }

    pub fn zyz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zyz()))
    }

    pub fn zzx(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zzx()))
    }

    pub fn zzy(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zzy()))
    }

    pub fn zzz(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.zzz()))
    }

    pub fn min_element(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_element())
    }

    pub fn max_element(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_element())
    }

    pub fn angle_between(&self, other: &PyVec3A) -> PyResult<f32> {
        Ok(self.as_ref()?.angle_between(*other.as_ref()?))
    }

    pub fn any_orthogonal_vector(&self) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.any_orthogonal_vector().into())
    }

    pub fn any_orthonormal_vector(&self) -> PyResult<PyVec3A> {
        let vector = self.as_ref()?;
        if !vector.is_normalized() {
            return Err(PyValueError::new_err(VECTOR_NOT_NORMALIZED));
        }
        Ok(vector.any_orthonormal_vector().into())
    }

    pub fn any_orthonormal_pair(&self) -> PyResult<(PyVec3A, PyVec3A)> {
        let vector = self.as_ref()?;
        if !vector.is_normalized() {
            return Err(PyValueError::new_err(VECTOR_NOT_NORMALIZED));
        }
        let (v1, v2) = vector.any_orthonormal_pair();
        Ok((PyVec3A::from_vec3a(v1), PyVec3A::from_vec3a(v2)))
    }

    pub fn length_recip(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_recip())
    }

    pub fn slerp(&self, rhs: &PyVec3A, s: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.slerp(*rhs.as_ref()?, s)))
    }

    pub fn clamp_length(&self, min: f32, max: f32) -> PyResult<PyVec3A> {
        let min = check_for::<Self, _>("min", min, NON_NEGATIVE)?;
        let max = check_for::<Self, _>("max", max, NON_NEGATIVE)?;
        if min > max {
            return Err(PyValueError::new_err(CLAMP_MIN_EXCEEDS_MAX));
        }
        Ok(PyVec3A::from_vec3a(self.as_ref()?.clamp_length(min, max)))
    }

    pub fn clamp_length_min(&self, min: f32) -> PyResult<PyVec3A> {
        let min = check_for::<Self, _>("min", min, NON_NEGATIVE)?;
        Ok(PyVec3A::from_vec3a(self.as_ref()?.clamp_length_min(min)))
    }

    pub fn clamp_length_max(&self, max: f32) -> PyResult<PyVec3A> {
        let max = check_for::<Self, _>("max", max, NON_NEGATIVE)?;
        Ok(PyVec3A::from_vec3a(self.as_ref()?.clamp_length_max(max)))
    }

    pub fn project_onto_normalized(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        let value = self.as_ref()?;
        let rhs = rhs.as_ref()?;
        if !rhs.is_normalized() {
            return Err(PyValueError::new_err(RHS_NOT_NORMALIZED));
        }
        Ok(PyVec3A::from_vec3a(value.project_onto_normalized(*rhs)))
    }

    pub fn reject_from_normalized(&self, rhs: &PyVec3A) -> PyResult<PyVec3A> {
        let value = self.as_ref()?;
        let rhs = rhs.as_ref()?;
        if !rhs.is_normalized() {
            return Err(PyValueError::new_err(RHS_NOT_NORMALIZED));
        }
        Ok(PyVec3A::from_vec3a(value.reject_from_normalized(*rhs)))
    }

    #[staticmethod]
    pub fn select(
        mask: (bool, bool, bool),
        if_true: &PyVec3A,
        if_false: &PyVec3A,
    ) -> PyResult<PyVec3A> {
        let bmask = BVec3A::new(mask.0, mask.1, mask.2);
        Ok(PyVec3A::from_vec3a(Vec3A::select(
            bmask,
            *if_true.as_ref()?,
            *if_false.as_ref()?,
        )))
    }

    pub fn cmpeq(&self, rhs: &PyVec3A) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmpeq(*rhs.as_ref()?);
        let [x, y, z]: [bool; 3] = result.into();
        Ok((x, y, z))
    }

    pub fn cmpne(&self, rhs: &PyVec3A) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmpne(*rhs.as_ref()?);
        let [x, y, z]: [bool; 3] = result.into();
        Ok((x, y, z))
    }

    pub fn cmpge(&self, rhs: &PyVec3A) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmpge(*rhs.as_ref()?);
        let [x, y, z]: [bool; 3] = result.into();
        Ok((x, y, z))
    }

    pub fn cmpgt(&self, rhs: &PyVec3A) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmpgt(*rhs.as_ref()?);
        let [x, y, z]: [bool; 3] = result.into();
        Ok((x, y, z))
    }

    pub fn cmple(&self, rhs: &PyVec3A) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmple(*rhs.as_ref()?);
        let [x, y, z]: [bool; 3] = result.into();
        Ok((x, y, z))
    }

    pub fn cmplt(&self, rhs: &PyVec3A) -> PyResult<(bool, bool, bool)> {
        let result = self.as_ref()?.cmplt(*rhs.as_ref()?);
        let [x, y, z]: [bool; 3] = result.into();
        Ok((x, y, z))
    }

    #[staticmethod]
    pub fn from_slice(slice: Vec<f32>) -> PyResult<PyVec3A> {
        if slice.len() < 3 {
            return Err(PyValueError::new_err(from_slice_too_short(3)));
        }
        Ok(PyVec3A::from_vec3a(Vec3A::from_slice(&slice)))
    }

    pub fn as_uvec3(&self) -> PyResult<PyUVec3> {
        Ok(self.as_ref()?.as_uvec3().into())
    }

    pub fn reflect(&self, normal: &PyVec3A) -> PyResult<PyVec3A> {
        let normal = *normal.as_ref()?;
        if !normal.is_normalized() {
            return Err(PyValueError::new_err(NORMAL_NOT_NORMALIZED));
        }
        Ok(PyVec3A::from_vec3a(self.as_ref()?.reflect(normal)))
    }

    pub fn refract(&self, normal: &PyVec3A, eta: f32) -> PyResult<PyVec3A> {
        let incident = *self.as_ref()?;
        let normal = *normal.as_ref()?;
        if !incident.is_normalized() {
            return Err(PyValueError::new_err(VECTOR_NOT_NORMALIZED));
        }
        if !normal.is_normalized() {
            return Err(PyValueError::new_err(NORMAL_NOT_NORMALIZED));
        }
        Ok(PyVec3A::from_vec3a(incident.refract(normal, eta)))
    }

    pub fn normalize_and_length(&self) -> PyResult<(PyVec3A, f32)> {
        let (normalized, length) = self.as_ref()?.normalize_and_length();
        Ok((PyVec3A::from_vec3a(normalized), length))
    }

    pub fn rotate_towards(&self, rhs: &PyVec3A, max_angle: f32) -> PyResult<PyVec3A> {
        let (from, to) = (*self.as_ref()?, *rhs.as_ref()?);
        // glam clamps by this angle and panics when it is NaN.
        if from.angle_between(to).is_nan() {
            return Err(PyValueError::new_err(ROTATE_TOWARDS_UNDEFINED_ANGLE));
        }
        Ok(PyVec3A::from_vec3a(from.rotate_towards(to, max_angle)))
    }

    pub fn rotate_x(&self, angle: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.rotate_x(angle)))
    }

    pub fn rotate_y(&self, angle: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.rotate_y(angle)))
    }

    pub fn rotate_z(&self, angle: f32) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.rotate_z(angle)))
    }

    pub fn rotate_axis(&self, axis: &PyVec3A, angle: f32) -> PyResult<PyVec3A> {
        let axis = *axis.as_ref()?;
        if !axis.is_normalized() {
            return Err(PyValueError::new_err(AXIS_NOT_NORMALIZED));
        }
        Ok(PyVec3A::from_vec3a(self.as_ref()?.rotate_axis(axis, angle)))
    }

    #[staticmethod]
    #[pyo3(name = "AXES")]
    pub fn axes() -> (Self, Self, Self) {
        (
            Self::vec3a(Vec3A::X),
            Self::vec3a(Vec3A::Y),
            Self::vec3a(Vec3A::Z),
        )
    }
}

/// Extract a `Vec3A` from the union Bevy accepts as `impl Into<Vec3A>`.
///
/// Bevy's bounding-volume and isometry constructors are generic over
/// `Into<Vec3A>`, which `Vec3` satisfies. Mirroring that here keeps a value read
/// back from one of these types usable as an argument to the next call.
pub fn extract_vec3a_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Vec3A> {
    if let Ok(value) = obj.extract::<PyVec3A>() {
        return value.try_into();
    }
    if let Ok(value) = obj.extract::<PyVec3>() {
        return Ok(Vec3A::from(Vec3::try_from(value)?));
    }
    Err(PyTypeError::new_err(format!(
        "expected Vec3A or Vec3, got {}",
        obj.get_type().name()?
    )))
}
