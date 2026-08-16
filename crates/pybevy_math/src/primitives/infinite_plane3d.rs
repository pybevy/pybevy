use bevy::math::primitives::InfinitePlane3d;
use pyo3::prelude::*;

use crate::{dir3::PyDir3, vec3::PyVec3};

#[pyclass(name = "InfinitePlane3d", frozen, eq, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub struct PyInfinitePlane3d {
    pub(crate) inner: InfinitePlane3d,
}

#[pymethods]
impl PyInfinitePlane3d {
    #[new]
    #[pyo3(signature = (normal = PyVec3::Y))]
    pub fn new(normal: PyVec3) -> PyResult<Self> {
        use bevy::math::Dir3;
        let dir = Dir3::new(normal.try_into()?)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{}", e)))?;
        Ok(Self {
            inner: InfinitePlane3d { normal: dir },
        })
    }

    #[staticmethod]
    pub fn from_dir(normal: PyDir3) -> PyResult<Self> {
        Ok(Self {
            inner: InfinitePlane3d {
                normal: normal.into_dir3()?,
            },
        })
    }

    #[getter]
    pub fn normal(&self) -> PyDir3 {
        PyDir3::from_dir3(self.inner.normal)
    }

    fn __repr__(&self) -> String {
        format!("InfinitePlane3d(normal={})", self.inner.normal)
    }
}

impl From<InfinitePlane3d> for PyInfinitePlane3d {
    fn from(plane: InfinitePlane3d) -> Self {
        Self { inner: plane }
    }
}

impl From<PyInfinitePlane3d> for InfinitePlane3d {
    fn from(plane: PyInfinitePlane3d) -> Self {
        plane.inner
    }
}
