use bevy::transform::components::{GlobalTransform, Transform};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::{
    affine3a::PyAffine3A, bounding::PyIsometry3d, mat4::PyMat4, quat::PyQuat, vec3::PyVec3,
};
use pyo3::prelude::*;

use crate::transform::PyTransform;

#[pycomponent(GlobalTransform, bridge, no_insert)]
#[pyclass(name = "GlobalTransform", extends = pybevy_core::PyComponent, eq)]
#[derive(Debug)]
pub struct PyGlobalTransform {
    pub(crate) storage: ComponentStorage<GlobalTransform>,
}

impl PartialEq for PyGlobalTransform {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyGlobalTransform {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (GlobalTransform::default().into(), PyComponent)
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    pub fn identity(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, (GlobalTransform::IDENTITY.into(), PyComponent))
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.translation().into())
    }

    #[getter]
    pub fn rotation(&self) -> PyResult<PyQuat> {
        Ok(self.as_ref()?.rotation().into())
    }

    #[getter]
    pub fn scale(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.scale().into())
    }

    pub fn to_scale_rotation_translation(&self) -> PyResult<(PyVec3, PyQuat, PyVec3)> {
        let (scale, rotation, translation) = self.as_ref()?.to_scale_rotation_translation();
        Ok((scale.into(), rotation.into(), translation.into()))
    }

    pub fn to_matrix(&self) -> PyResult<PyMat4> {
        Ok(self.as_ref()?.to_matrix().into())
    }

    pub fn affine(&self) -> PyResult<PyAffine3A> {
        Ok(self.as_ref()?.affine().into())
    }

    pub fn to_isometry(&self) -> PyResult<PyIsometry3d> {
        Ok(self.as_ref()?.to_isometry().into())
    }

    pub fn compute_transform(&self, py: Python<'_>) -> PyResult<Py<PyTransform>> {
        let transform = self.as_ref()?.compute_transform();
        Py::new(py, PyTransform::from_owned(transform))
    }

    pub fn transform_point(&self, point: PyVec3) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.transform_point(point.into()).into())
    }

    pub fn reparented_to(
        &self,
        py: Python<'_>,
        parent: &PyGlobalTransform,
    ) -> PyResult<Py<PyTransform>> {
        let transform = self.as_ref()?.reparented_to(parent.as_ref()?);
        Py::new(py, PyTransform::from_owned(transform))
    }

    pub fn right(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.right().into())
    }

    pub fn left(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.left().into())
    }

    pub fn up(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.up().into())
    }

    pub fn down(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.down().into())
    }

    pub fn forward(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.forward().into())
    }

    pub fn back(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.back().into())
    }

    pub fn mul_transform(
        &self,
        py: Python<'_>,
        transform: &PyTransform,
    ) -> PyResult<Py<PyGlobalTransform>> {
        let bevy_transform: Transform = transform.as_ref()?.clone();
        let result = self.as_ref()?.mul_transform(bevy_transform);
        Py::new(py, PyGlobalTransform::from_owned(result))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref() {
            Ok(gt) => {
                let t = gt.translation();
                let r = gt.rotation();
                let s = gt.scale();
                Ok(format!(
                    "GlobalTransform(translation={:?}, rotation={:?}, scale={:?})",
                    t, r, s
                ))
            }
            Err(_) => Ok("GlobalTransform(<invalid>)".to_string()),
        }
    }
}
