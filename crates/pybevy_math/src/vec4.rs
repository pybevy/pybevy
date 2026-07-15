use bevy::math::{BVec4A, Vec4, Vec4Swizzles};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use crate::vec3::PyVec3;

fn bool_tuple(value: BVec4A) -> (bool, bool, bool, bool) {
    let mask = value.bitmask();
    (mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0)
}

#[pyclass(name = "Vec4", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyVec4 {
    pub(crate) storage: ValueStorage<Vec4>,
}

impl From<PyVec4> for Vec4 {
    #[inline(always)]
    fn from(py_vec: PyVec4) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<&PyVec4> for Vec4 {
    #[inline(always)]
    fn from(py_vec: &PyVec4) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<Vec4> for PyVec4 {
    #[inline(always)]
    fn from(vec: Vec4) -> Self {
        PyVec4::from_vec4(vec)
    }
}

impl FromBorrowedStorage<ValueStorage<Vec4>> for PyVec4 {
    fn from_borrowed(storage: ValueStorage<Vec4>) -> Self {
        PyVec4 { storage }
    }
}

impl PyVec4 {
    #[inline(always)]
    pub fn from_vec4(vec: Vec4) -> Self {
        PyVec4 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    pub fn into_vec4(self) -> Vec4 {
        self.into()
    }

    #[inline(always)]
    pub const fn vec4(vec: Vec4) -> Self {
        PyVec4 {
            storage: ValueStorage::owned(vec),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Vec4> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Vec4> {
        Ok(self.storage.as_mut()?)
    }

    pub const ZERO: PyVec4 = PyVec4::vec4(Vec4::ZERO);
    pub const ONE: PyVec4 = PyVec4::vec4(Vec4::ONE);
    pub const NEG_ONE: PyVec4 = PyVec4::vec4(Vec4::NEG_ONE);
    pub const MIN: PyVec4 = PyVec4::vec4(Vec4::MIN);
    pub const MAX: PyVec4 = PyVec4::vec4(Vec4::MAX);
    pub const NAN: PyVec4 = PyVec4::vec4(Vec4::NAN);
    pub const INFINITY: PyVec4 = PyVec4::vec4(Vec4::INFINITY);
    pub const NEG_INFINITY: PyVec4 = PyVec4::vec4(Vec4::NEG_INFINITY);
    pub const X: PyVec4 = PyVec4::vec4(Vec4::X);
    pub const Y: PyVec4 = PyVec4::vec4(Vec4::Y);
    pub const Z: PyVec4 = PyVec4::vec4(Vec4::Z);
    pub const W: PyVec4 = PyVec4::vec4(Vec4::W);
    pub const NEG_X: PyVec4 = PyVec4::vec4(Vec4::NEG_X);
    pub const NEG_Y: PyVec4 = PyVec4::vec4(Vec4::NEG_Y);
    pub const NEG_Z: PyVec4 = PyVec4::vec4(Vec4::NEG_Z);
    pub const NEG_W: PyVec4 = PyVec4::vec4(Vec4::NEG_W);
}

#[pymethods]
impl PyVec4 {
    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::vec4(Vec4::ZERO)
    }
    #[staticmethod]
    #[pyo3(name = "ONE")]
    pub fn one() -> Self {
        Self::vec4(Vec4::ONE)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_ONE")]
    pub fn neg_one() -> Self {
        Self::vec4(Vec4::NEG_ONE)
    }
    #[staticmethod]
    #[pyo3(name = "MIN")]
    pub fn min_value() -> Self {
        Self::vec4(Vec4::MIN)
    }
    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max_value() -> Self {
        Self::vec4(Vec4::MAX)
    }
    #[staticmethod]
    #[pyo3(name = "NAN")]
    pub fn nan() -> Self {
        Self::vec4(Vec4::NAN)
    }
    #[staticmethod]
    #[pyo3(name = "INFINITY")]
    pub fn infinity() -> Self {
        Self::vec4(Vec4::INFINITY)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_INFINITY")]
    pub fn neg_infinity() -> Self {
        Self::vec4(Vec4::NEG_INFINITY)
    }
    #[staticmethod]
    #[pyo3(name = "X")]
    pub fn unit_x() -> Self {
        Self::vec4(Vec4::X)
    }
    #[staticmethod]
    #[pyo3(name = "Y")]
    pub fn unit_y() -> Self {
        Self::vec4(Vec4::Y)
    }
    #[staticmethod]
    #[pyo3(name = "Z")]
    pub fn unit_z() -> Self {
        Self::vec4(Vec4::Z)
    }
    #[staticmethod]
    #[pyo3(name = "W")]
    pub fn unit_w() -> Self {
        Self::vec4(Vec4::W)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_X")]
    pub fn neg_x() -> Self {
        Self::vec4(Vec4::NEG_X)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_Y")]
    pub fn neg_y() -> Self {
        Self::vec4(Vec4::NEG_Y)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_Z")]
    pub fn neg_z() -> Self {
        Self::vec4(Vec4::NEG_Z)
    }
    #[staticmethod]
    #[pyo3(name = "NEG_W")]
    pub fn neg_w() -> Self {
        Self::vec4(Vec4::NEG_W)
    }

    #[new]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        PyVec4::vec4(Vec4::new(x, y, z, w))
    }

    #[staticmethod]
    pub fn splat(value: f32) -> Self {
        PyVec4::vec4(Vec4::splat(value))
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

    #[getter]
    pub fn w(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.w)
    }

    #[setter]
    pub fn set_w(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.w = value;
        Ok(())
    }

    pub fn dot(&self, other: &PyVec4) -> PyResult<f32> {
        Ok(self.as_ref()?.dot(*other.as_ref()?))
    }

    pub fn length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length())
    }

    pub fn length_squared(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_squared())
    }

    pub fn length_recip(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_recip())
    }

    pub fn normalize(&self) -> PyResult<Self> {
        Ok(PyVec4::from_vec4(self.as_ref()?.normalize()))
    }

    pub fn truncate(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.truncate().into())
    }

    #[staticmethod]
    pub fn from_array(a: (f32, f32, f32, f32)) -> PyVec4 {
        PyVec4::from_vec4(Vec4::from_array([a.0, a.1, a.2, a.3]))
    }

    #[staticmethod]
    pub fn from_slice(a: Vec<f32>) -> PyResult<PyVec4> {
        if a.len() < 4 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "from_slice requires at least 4 elements",
            ));
        }
        Ok(PyVec4::from_vec4(Vec4::from_slice(&a)))
    }

    #[staticmethod]
    pub fn select(
        mask: (bool, bool, bool, bool),
        if_true: &PyVec4,
        if_false: &PyVec4,
    ) -> PyResult<PyVec4> {
        let mask = BVec4A::new(mask.0, mask.1, mask.2, mask.3);
        Ok(PyVec4::from_vec4(Vec4::select(
            mask,
            *if_true.as_ref()?,
            *if_false.as_ref()?,
        )))
    }

    pub fn to_array(&self) -> PyResult<(f32, f32, f32, f32)> {
        let arr = self.as_ref()?.to_array();
        Ok((arr[0], arr[1], arr[2], arr[3]))
    }

    pub fn abs(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.abs()))
    }

    pub fn signum(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.signum()))
    }

    pub fn copysign(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.copysign(*rhs.as_ref()?)))
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn round(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.round()))
    }

    pub fn trunc(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.trunc()))
    }

    pub fn fract(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.fract()))
    }

    pub fn fract_gl(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.fract_gl()))
    }

    pub fn exp(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.exp()))
    }

    pub fn powf(&self, n: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.powf(n)))
    }

    pub fn recip(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.recip()))
    }

    pub fn lerp(&self, rhs: &PyVec4, s: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.lerp(*rhs.as_ref()?, s)))
    }

    pub fn move_towards(&self, rhs: &PyVec4, d: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(
            self.as_ref()?.move_towards(*rhs.as_ref()?, d),
        ))
    }

    pub fn midpoint(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.midpoint(*rhs.as_ref()?)))
    }

    pub fn project_onto(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(
            self.as_ref()?.project_onto(*rhs.as_ref()?),
        ))
    }

    pub fn reject_from(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(
            self.as_ref()?.reject_from(*rhs.as_ref()?),
        ))
    }

    pub fn project_onto_normalized(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(
            self.as_ref()?.project_onto_normalized(*rhs.as_ref()?),
        ))
    }

    pub fn reject_from_normalized(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(
            self.as_ref()?.reject_from_normalized(*rhs.as_ref()?),
        ))
    }

    pub fn normalize_or_zero(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.normalize_or_zero()))
    }

    pub fn try_normalize(&self) -> PyResult<Option<PyVec4>> {
        Ok(self.as_ref()?.try_normalize().map(PyVec4::from_vec4))
    }

    pub fn distance(&self, rhs: &PyVec4) -> PyResult<f32> {
        Ok(self.as_ref()?.distance(*rhs.as_ref()?))
    }

    pub fn distance_squared(&self, rhs: &PyVec4) -> PyResult<f32> {
        Ok(self.as_ref()?.distance_squared(*rhs.as_ref()?))
    }

    pub fn min(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.min(*rhs.as_ref()?)))
    }

    pub fn max(&self, rhs: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.max(*rhs.as_ref()?)))
    }

    pub fn clamp(&self, min: &PyVec4, max: &PyVec4) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(
            self.as_ref()?.clamp(*min.as_ref()?, *max.as_ref()?),
        ))
    }

    pub fn clamp_length(&self, min: f32, max: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.clamp_length(min, max)))
    }

    pub fn clamp_length_min(&self, min: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.clamp_length_min(min)))
    }

    pub fn clamp_length_max(&self, max: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.clamp_length_max(max)))
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

    pub fn floor(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.floor()))
    }

    pub fn ceil(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.ceil()))
    }

    pub fn min_element(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_element())
    }

    pub fn max_element(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_element())
    }

    pub fn xyz(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.xyz().into())
    }

    pub fn xyzw(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.xyzw()))
    }

    pub fn with_x(&self, x: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.with_x(x)))
    }
    pub fn with_y(&self, y: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.with_y(y)))
    }
    pub fn with_z(&self, z: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.with_z(z)))
    }
    pub fn with_w(&self, w: f32) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(self.as_ref()?.with_w(w)))
    }

    pub fn cmpeq(&self, rhs: &PyVec4) -> PyResult<(bool, bool, bool, bool)> {
        Ok(bool_tuple(self.as_ref()?.cmpeq(*rhs.as_ref()?)))
    }
    pub fn cmpne(&self, rhs: &PyVec4) -> PyResult<(bool, bool, bool, bool)> {
        Ok(bool_tuple(self.as_ref()?.cmpne(*rhs.as_ref()?)))
    }
    pub fn cmplt(&self, rhs: &PyVec4) -> PyResult<(bool, bool, bool, bool)> {
        Ok(bool_tuple(self.as_ref()?.cmplt(*rhs.as_ref()?)))
    }
    pub fn cmple(&self, rhs: &PyVec4) -> PyResult<(bool, bool, bool, bool)> {
        Ok(bool_tuple(self.as_ref()?.cmple(*rhs.as_ref()?)))
    }
    pub fn cmpgt(&self, rhs: &PyVec4) -> PyResult<(bool, bool, bool, bool)> {
        Ok(bool_tuple(self.as_ref()?.cmpgt(*rhs.as_ref()?)))
    }
    pub fn cmpge(&self, rhs: &PyVec4) -> PyResult<(bool, bool, bool, bool)> {
        Ok(bool_tuple(self.as_ref()?.cmpge(*rhs.as_ref()?)))
    }

    pub fn xx(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.x, v.x))
    }
    pub fn xy(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.x, v.y))
    }
    pub fn xz(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.x, v.z))
    }
    pub fn xw(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.x, v.w))
    }
    pub fn yx(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.y, v.x))
    }
    pub fn yy(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.y, v.y))
    }
    pub fn yz(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.y, v.z))
    }
    pub fn yw(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.y, v.w))
    }
    pub fn zx(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.z, v.x))
    }
    pub fn zy(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.z, v.y))
    }
    pub fn zz(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.z, v.z))
    }
    pub fn zw(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.z, v.w))
    }
    pub fn wx(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.w, v.x))
    }
    pub fn wy(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.w, v.y))
    }
    pub fn wz(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.w, v.z))
    }
    pub fn ww(&self) -> PyResult<(f32, f32)> {
        let v = self.as_ref()?;
        Ok((v.w, v.w))
    }

    fn __add__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec4::from_vec4(self_vec + scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec4>() {
            Ok(Py::new(py, PyVec4::from_vec4(self_vec + *other_vec.as_ref()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __sub__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec4::from_vec4(self_vec - scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec4>() {
            Ok(Py::new(py, PyVec4::from_vec4(self_vec - *other_vec.as_ref()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec4::from_vec4(self_vec * scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec4>() {
            Ok(Py::new(py, PyVec4::from_vec4(self_vec * *other_vec.as_ref()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __div__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec4::from_vec4(self_vec / scalar))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec4>() {
            Ok(Py::new(py, PyVec4::from_vec4(self_vec / *other_vec.as_ref()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __truediv__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.__div__(other, py)
    }

    fn __neg__(&self) -> PyResult<PyVec4> {
        Ok(PyVec4::from_vec4(-*self.as_ref()?))
    }

    fn __rmul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let self_vec = *self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec4::from_vec4(scalar * self_vec))?.into_any())
        } else if let Ok(other_vec) = other.extract::<PyVec4>() {
            Ok(Py::new(py, PyVec4::from_vec4(*other_vec.as_ref()? * self_vec))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        let v = *self.as_ref()?;
        Ok(format!("Vec4({}, {}, {}, {})", v.x, v.y, v.z, v.w))
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_vec) = other.extract::<PyVec4>() {
            let self_vec = *self.as_ref()?;
            let other_val = *other_vec.as_ref()?;
            match op {
                CompareOp::Eq => Ok(self_vec == other_val),
                CompareOp::Ne => Ok(self_vec != other_val),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else if let Ok(tuple) = other.extract::<(f32, f32, f32, f32)>() {
            match op {
                CompareOp::Eq => Ok(self.as_tuple()? == tuple),
                CompareOp::Ne => Ok(self.as_tuple()? != tuple),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare Vec4 with another Vec4 or a tuple of four floats",
            ))
        }
    }

    fn as_tuple(&self) -> PyResult<(f32, f32, f32, f32)> {
        let v = *self.as_ref()?;
        Ok((v.x, v.y, v.z, v.w))
    }
}
