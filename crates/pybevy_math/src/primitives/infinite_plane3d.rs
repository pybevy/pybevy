use bevy::math::{Isometry3d, Vec3, Vec3A, primitives::InfinitePlane3d};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    bounding::aabb3d::PyIsometry3d, dir3::PyDir3, quat::PyQuat, vec3::PyVec3, vec3a::PyVec3A,
};

/// Accepts an Isometry3d, Vec3, Vec3A, or Quat, matching bevy's `impl Into<Isometry3d>`.
fn extract_isometry3d_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Isometry3d> {
    if let Ok(iso) = obj.extract::<PyIsometry3d>() {
        return iso.try_into();
    }
    if let Ok(vec) = obj.extract::<PyVec3>() {
        let vec: Vec3 = vec.try_into()?;
        return Ok(Isometry3d::from(vec));
    }
    if let Ok(vec_a) = obj.extract::<PyVec3A>() {
        let vec_a: Vec3A = vec_a.try_into()?;
        return Ok(Isometry3d::from(vec_a));
    }
    if let Ok(quat) = obj.extract::<PyQuat>() {
        let quat: bevy::math::Quat = quat.try_into()?;
        return Ok(Isometry3d::from(quat));
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "Expected Isometry3d, Vec3, Vec3A, or Quat",
    ))
}

#[pyclass(
    name = "InfinitePlane3d",
    module = "pybevy.math",
    frozen,
    eq,
    skip_from_py_object
)]
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
        let dir =
            Dir3::new(normal.try_into()?).map_err(|e| PyValueError::new_err(format!("{}", e)))?;
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

    #[staticmethod]
    pub fn from_points(a: PyVec3, b: PyVec3, c: PyVec3) -> PyResult<(Self, PyVec3)> {
        let (plane, origin) =
            InfinitePlane3d::from_points(a.try_into()?, b.try_into()?, c.try_into()?);
        Ok((Self { inner: plane }, origin.into()))
    }

    pub fn signed_distance(&self, isometry: &Bound<'_, PyAny>, point: PyVec3) -> PyResult<f32> {
        let iso = extract_isometry3d_from_any(isometry)?;
        Ok(self.inner.signed_distance(iso, point.try_into()?))
    }

    pub fn project_point(&self, isometry: &Bound<'_, PyAny>, point: PyVec3) -> PyResult<PyVec3> {
        let iso = extract_isometry3d_from_any(isometry)?;
        Ok(self.inner.project_point(iso, point.try_into()?).into())
    }

    pub fn isometry_into_xy(&self, origin: PyVec3) -> PyResult<PyIsometry3d> {
        Ok(self.inner.isometry_into_xy(origin.try_into()?).into())
    }

    pub fn isometry_from_xy(&self, origin: PyVec3) -> PyResult<PyIsometry3d> {
        Ok(self.inner.isometry_from_xy(origin.try_into()?).into())
    }

    pub fn isometries_xy(&self, origin: PyVec3) -> PyResult<(PyIsometry3d, PyIsometry3d)> {
        let (into_xy, from_xy) = self.inner.isometries_xy(origin.try_into()?);
        Ok((into_xy.into(), from_xy.into()))
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
