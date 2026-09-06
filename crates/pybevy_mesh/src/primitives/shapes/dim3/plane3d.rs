use bevy::{
    math::{Dir3, Vec3, primitives::Plane3d},
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::{vec2::PyVec2, vec3::PyVec3};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyPlaneMeshBuilder};

#[pyvalue]
#[pyclass(name = "Plane3d", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyPlane3d {
    pub(crate) storage: ValueStorage<Plane3d>,
}

impl PartialEq for PyPlane3d {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyPlane3d {
    #[new]
    #[pyo3(signature = (
        normal = PyVec3::Y,
        half_size = PyVec2::splat(0.5)
    ))]
    pub fn new(normal: PyVec3, half_size: PyVec2) -> PyResult<PyClassInitializer<Self>> {
        let normal = validate_normal(normal.try_into()?)?;
        Ok((
            Self::from_owned(Plane3d {
                normal,
                half_size: half_size.try_into()?,
            }),
            PyMeshable,
        )
            .into())
    }

    #[staticmethod]
    pub fn from_points(
        py: Python<'_>,
        a: PyVec3,
        b: PyVec3,
        c: PyVec3,
    ) -> PyResult<(Py<PyPlane3d>, PyVec3)> {
        let a: Vec3 = a.try_into()?;
        let b: Vec3 = b.try_into()?;
        let c: Vec3 = c.try_into()?;
        let normal = Dir3::new((b - a).cross(c - a)).map_err(|_| {
            PyValueError::new_err(
                "finite plane must be defined by three finite, non-collinear points",
            )
        })?;
        let plane = Plane3d {
            normal,
            ..Default::default()
        };
        let translation = (a + b + c) / 3.0;
        let py_plane = Py::new(py, (PyPlane3d::from_owned(plane), PyMeshable))?;
        Ok((py_plane, PyVec3::from_vec3(translation)))
    }

    #[getter]
    pub fn half_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.half_size)?)
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.half_size = value.try_into()?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Plane3d(normal={:?}, half_size={:?})",
            self.as_ref()?.normal,
            self.as_ref()?.half_size
        ))
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyPlaneMeshBuilder>> {
        Py::new(py, (self.as_ref()?.mesh().into(), PyMeshBuilder))
    }
}

fn validate_normal(normal: Vec3) -> PyResult<Dir3> {
    Dir3::new(normal).map_err(|error| PyValueError::new_err(error.to_string()))
}

impl From<Plane3d> for PyPlane3d {
    fn from(plane: Plane3d) -> Self {
        PyPlane3d::from_owned(plane)
    }
}
