use bevy::math::{Vec3, Vec3A};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use crate::vec3::PyVec3;

#[pyclass(name = "Vec3A", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyVec3A {
    storage: ValueStorage<Vec3A>,
}

impl From<PyVec3A> for Vec3A {
    #[inline(always)]
    fn from(py_vec: PyVec3A) -> Self {
        py_vec.storage.get().unwrap()
    }
}

impl From<&PyVec3A> for Vec3A {
    #[inline(always)]
    fn from(py_vec: &PyVec3A) -> Self {
        py_vec.storage.get().unwrap()
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
    fn as_ref(&self) -> PyResult<&Vec3A> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Vec3A> {
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
    pub fn from_vec3(v: &PyVec3) -> Self {
        PyVec3A::from_vec3a(Vec3A::from(v.get()))
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

    pub fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        let s = self.as_ref()?;
        let o = other.as_ref()?;
        match op {
            CompareOp::Eq => Ok(*s == *o),
            CompareOp::Ne => Ok(*s != *o),
            _ => Err(PyTypeError::new_err("Vec3A only supports == and !=")),
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
