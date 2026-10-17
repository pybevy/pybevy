use bevy::{math::Vec4, shape::HalfSpace};
use pybevy_core::{Omittable, repr::ReprLiteral};
use pyo3::prelude::*;

use crate::{vec3::PyVec3, vec3a::PyVec3A, vec4::PyVec4};

#[pyclass(name = "HalfSpace", module = "pybevy.shape", frozen, from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyHalfSpace {
    pub(crate) inner: HalfSpace,
}

impl From<HalfSpace> for PyHalfSpace {
    fn from(hs: HalfSpace) -> Self {
        PyHalfSpace { inner: hs }
    }
}

impl From<PyHalfSpace> for HalfSpace {
    fn from(hs: PyHalfSpace) -> Self {
        hs.inner
    }
}

impl From<&PyHalfSpace> for HalfSpace {
    fn from(hs: &PyHalfSpace) -> Self {
        hs.inner
    }
}

#[pymethods]
impl PyHalfSpace {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(*self)
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (normal_d = Omittable::default()))]
    pub fn new(normal_d: Omittable<PyRef<'_, PyVec4>>) -> PyResult<Self> {
        let inner = match normal_d.into_option() {
            Some(normal_d) => {
                let vec4: Vec4 = (&*normal_d).try_into()?;
                HalfSpace::new(vec4)
            }
            None => HalfSpace::default(),
        };
        Ok(PyHalfSpace { inner })
    }

    pub fn normal(&self) -> PyVec3A {
        PyVec3A::from_vec3a(self.inner.normal())
    }

    pub fn d(&self) -> f32 {
        self.inner.d()
    }

    pub fn normal_d(&self) -> PyVec4 {
        PyVec4::from_vec4(self.inner.normal_d())
    }

    #[staticmethod]
    pub fn intersection_point(a: &PyHalfSpace, b: &PyHalfSpace, c: &PyHalfSpace) -> Option<PyVec3> {
        HalfSpace::intersection_point(a.inner, b.inner, c.inner).map(PyVec3::from_vec3)
    }

    pub fn __repr__(&self) -> String {
        let nd = self.inner.normal_d();
        format!("HalfSpace(normal_d={})", nd.repr_literal())
    }

    pub fn __eq__(&self, other: &Self) -> bool {
        self.inner.normal_d() == other.inner.normal_d()
    }
}
