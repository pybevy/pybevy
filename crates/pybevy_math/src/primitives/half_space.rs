use bevy::math::{Vec4, primitives::HalfSpace};
use pyo3::prelude::*;

use crate::vec4::PyVec4;

#[pyclass(name = "HalfSpace")]
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
    #[new]
    pub fn new(normal_d: &PyVec4) -> Self {
        let vec4: Vec4 = normal_d.into();
        PyHalfSpace {
            inner: HalfSpace::new(vec4),
        }
    }

    #[getter]
    pub fn normal(&self) -> PyVec4 {
        // HalfSpace.normal() returns Vec3A, but we return Vec4 with w=0 for simplicity
        let n = self.inner.normal();
        PyVec4::from_vec4(Vec4::new(n.x, n.y, n.z, 0.0))
    }

    #[getter]
    pub fn d(&self) -> f32 {
        self.inner.d()
    }

    #[getter]
    pub fn normal_d(&self) -> PyVec4 {
        PyVec4::from_vec4(self.inner.normal_d())
    }

    pub fn __repr__(&self) -> String {
        let nd = self.inner.normal_d();
        format!(
            "HalfSpace(normal_d=Vec4({}, {}, {}, {}))",
            nd.x, nd.y, nd.z, nd.w
        )
    }

    pub fn __eq__(&self, other: &Self) -> bool {
        self.inner.normal_d() == other.inner.normal_d()
    }
}
