use bevy::math::{Dir2, primitives::Plane2d};
use pyo3::prelude::*;

use crate::{dir2::PyDir2, vec2::PyVec2};

#[pyclass(name = "Plane2d", eq, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub struct PyPlane2d {
    pub(crate) inner: Plane2d,
}

#[pymethods]
impl PyPlane2d {
    #[new]
    #[pyo3(signature = (normal = PyVec2::Y))]
    pub fn new(normal: PyVec2) -> PyResult<Self> {
        let dir = Dir2::new(normal.try_get()?)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{}", e)))?;
        Ok(Self {
            inner: Plane2d { normal: dir },
        })
    }

    #[staticmethod]
    pub fn from_dir(normal: PyDir2) -> PyResult<Self> {
        Ok(Self {
            inner: Plane2d {
                normal: normal.into_dir2()?,
            },
        })
    }

    #[getter]
    pub fn normal(&self) -> PyDir2 {
        PyDir2::from_dir2(self.inner.normal)
    }

    #[setter]
    pub fn set_normal(&mut self, normal: PyDir2) -> PyResult<()> {
        self.inner.normal = normal.into_dir2()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("Plane2d(normal={})", self.inner.normal)
    }
}

impl From<Plane2d> for PyPlane2d {
    fn from(plane: Plane2d) -> Self {
        Self { inner: plane }
    }
}

impl From<PyPlane2d> for Plane2d {
    fn from(plane: PyPlane2d) -> Self {
        plane.inner
    }
}
