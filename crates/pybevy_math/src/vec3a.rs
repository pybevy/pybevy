use bevy::math::{Vec3, Vec3A};
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use crate::vec3::PyVec3;

#[pyclass(name = "Vec3A", from_py_object)]
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

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Vec3A>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyVec3A {
    #[classattr]
    pub const ZERO: PyVec3A = PyVec3A::vec3a(Vec3A::ZERO);

    #[classattr]
    pub const ONE: PyVec3A = PyVec3A::vec3a(Vec3A::ONE);

    #[classattr]
    pub const NEG_ONE: PyVec3A = PyVec3A::vec3a(Vec3A::NEG_ONE);

    #[classattr]
    pub const MIN: PyVec3A = PyVec3A::vec3a(Vec3A::MIN);

    #[classattr]
    pub const MAX: PyVec3A = PyVec3A::vec3a(Vec3A::MAX);

    #[classattr]
    pub const NAN: PyVec3A = PyVec3A::vec3a(Vec3A::NAN);

    #[classattr]
    pub const INFINITY: PyVec3A = PyVec3A::vec3a(Vec3A::INFINITY);

    #[classattr]
    pub const NEG_INFINITY: PyVec3A = PyVec3A::vec3a(Vec3A::NEG_INFINITY);

    #[classattr]
    pub const X: PyVec3A = PyVec3A::vec3a(Vec3A::X);

    #[classattr]
    pub const Y: PyVec3A = PyVec3A::vec3a(Vec3A::Y);

    #[classattr]
    pub const Z: PyVec3A = PyVec3A::vec3a(Vec3A::Z);

    #[classattr]
    pub const NEG_X: PyVec3A = PyVec3A::vec3a(Vec3A::NEG_X);

    #[classattr]
    pub const NEG_Y: PyVec3A = PyVec3A::vec3a(Vec3A::NEG_Y);

    #[classattr]
    pub const NEG_Z: PyVec3A = PyVec3A::vec3a(Vec3A::NEG_Z);

    #[new]
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

    pub fn __add__(&self, other: &PyVec3A) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(*self.as_ref()? + *other.as_ref()?))
    }

    pub fn __sub__(&self, other: &PyVec3A) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(*self.as_ref()? - *other.as_ref()?))
    }

    pub fn __mul__(&self, scalar: f32) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(*self.as_ref()? * scalar))
    }

    pub fn __rmul__(&self, scalar: f32) -> PyResult<Self> {
        self.__mul__(scalar)
    }

    pub fn __truediv__(&self, scalar: f32) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(*self.as_ref()? / scalar))
    }

    pub fn __neg__(&self) -> PyResult<Self> {
        Ok(PyVec3A::from_vec3a(-*self.as_ref()?))
    }

    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other = if let Ok(other_vec) = other.extract::<PyVec3A>() {
            *other_vec.as_ref()?
        } else if let Ok((x, y, z)) = other.extract::<(f32, f32, f32)>() {
            Vec3A::new(x, y, z)
        } else {
            return Err(PyTypeError::new_err(
                "Can only compare Vec3A with another Vec3A or a tuple of three floats",
            ));
        };

        match op {
            CompareOp::Eq => Ok(*self.as_ref()? == other),
            CompareOp::Ne => Ok(*self.as_ref()? != other),
            _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
        }
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let v = self.as_ref()?;
        Ok(format!("Vec3A({}, {}, {})", v.x, v.y, v.z))
    }

    pub fn __str__(&self) -> PyResult<String> {
        self.__repr__()
    }
}

/// Extract a `Vec3A` from the union Bevy accepts as `impl Into<Vec3A>`.
///
/// Bevy's bounding-volume and isometry constructors are generic over
/// `Into<Vec3A>`, which `Vec3` satisfies. Mirroring that here keeps a value read
/// back from one of these types usable as an argument to the next call.
pub fn extract_vec3a_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Vec3A> {
    if let Ok(value) = obj.extract::<PyVec3A>() {
        return Ok(value.try_into()?);
    }
    if let Ok(value) = obj.extract::<PyVec3>() {
        return Ok(Vec3A::from(Vec3::try_from(value)?));
    }
    Err(PyTypeError::new_err(format!(
        "expected Vec3A or Vec3, got {}",
        obj.get_type().name()?
    )))
}
