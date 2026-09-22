use bevy::math::{Dir3, Isometry3d, Quat, Vec3, Vec3A, primitives::InfinitePlane3d};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    public_error::{INFINITE_PLANE_POINTS, UNSUPPORTED_COMPARISON},
};
use pybevy_macros::pyvalue;
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::{
    bounding::aabb3d::PyIsometry3d, dir3::PyDir3, quat::PyQuat, richcmp::comparison_result,
    vec3::PyVec3, vec3a::PyVec3A,
};

/// Correct Bevy's rotated-plane projection until it subtracts the world-space
/// normal (still present at bevyengine/bevy@70c2b021).
fn infinite_plane_project_point(
    plane: &InfinitePlane3d,
    isometry: Isometry3d,
    point: Vec3,
) -> Vec3 {
    let world_normal = isometry * plane.normal;
    point - world_normal * plane.signed_distance(isometry, point)
}

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
        let quat: Quat = quat.try_into()?;
        return Ok(Isometry3d::from(quat));
    }
    Err(PyTypeError::new_err(
        "Expected Isometry3d, Vec3, Vec3A, or Quat",
    ))
}

#[pyvalue]
#[pyclass(name = "InfinitePlane3d", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyInfinitePlane3d {
    pub(crate) storage: ValueStorage<InfinitePlane3d>,
}

#[pymethods]
impl PyInfinitePlane3d {
    #[new]
    #[pyo3(signature = (*, normal = PyVec3::Y))]
    pub fn new(normal: PyVec3) -> PyResult<Self> {
        let dir =
            Dir3::new(normal.try_into()?).map_err(|e| PyValueError::new_err(format!("{}", e)))?;
        Ok(Self::from_owned(InfinitePlane3d { normal: dir }))
    }

    #[staticmethod]
    pub fn from_dir(normal: PyDir3) -> PyResult<Self> {
        Ok(Self::from_owned(InfinitePlane3d {
            normal: normal.into_dir3()?,
        }))
    }

    #[staticmethod]
    pub fn from_points(a: PyVec3, b: PyVec3, c: PyVec3) -> PyResult<(Self, PyVec3)> {
        let a: Vec3 = a.try_into()?;
        let b: Vec3 = b.try_into()?;
        let c: Vec3 = c.try_into()?;
        let normal = Dir3::new((b - a).cross(c - a))
            .map_err(|_| PyValueError::new_err(INFINITE_PLANE_POINTS))?;
        let origin = (a + b + c) / 3.0;
        Ok((Self::from_owned(InfinitePlane3d { normal }), origin.into()))
    }

    pub fn signed_distance(&self, isometry: &Bound<'_, PyAny>, point: PyVec3) -> PyResult<f32> {
        let iso = extract_isometry3d_from_any(isometry)?;
        Ok(self.as_ref()?.signed_distance(iso, point.try_into()?))
    }

    pub fn project_point(&self, isometry: &Bound<'_, PyAny>, point: PyVec3) -> PyResult<PyVec3> {
        let iso = extract_isometry3d_from_any(isometry)?;
        let plane = self.as_ref()?;
        Ok(infinite_plane_project_point(&plane, iso, point.try_into()?).into())
    }

    pub fn isometry_into_xy(&self, origin: PyVec3) -> PyResult<PyIsometry3d> {
        Ok(self.as_ref()?.isometry_into_xy(origin.try_into()?).into())
    }

    pub fn isometry_from_xy(&self, origin: PyVec3) -> PyResult<PyIsometry3d> {
        Ok(self.as_ref()?.isometry_from_xy(origin.try_into()?).into())
    }

    pub fn isometries_xy(&self, origin: PyVec3) -> PyResult<(PyIsometry3d, PyIsometry3d)> {
        let (into_xy, from_xy) = self.as_ref()?.isometries_xy(origin.try_into()?);
        Ok((into_xy.into(), from_xy.into()))
    }

    #[getter]
    pub fn normal(&self) -> PyResult<PyDir3> {
        Ok(self.storage.borrow_field_as(|plane| &plane.normal)?)
    }

    #[setter]
    pub fn set_normal(&mut self, normal: PyDir3) -> PyResult<()> {
        self.as_mut()?.normal = normal.into_dir3()?;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("InfinitePlane3d(normal={})", self.as_ref()?.normal))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyRef<'_, PyInfinitePlane3d>>() else {
            return Ok(py.NotImplemented());
        };
        let result = match op {
            CompareOp::Eq => *self.as_ref()? == *other_value.as_ref()?,
            CompareOp::Ne => *self.as_ref()? != *other_value.as_ref()?,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }
}

impl From<InfinitePlane3d> for PyInfinitePlane3d {
    fn from(plane: InfinitePlane3d) -> Self {
        Self::from_owned(plane)
    }
}

impl TryFrom<PyInfinitePlane3d> for InfinitePlane3d {
    type Error = PyErr;

    fn try_from(plane: PyInfinitePlane3d) -> PyResult<Self> {
        plane.to_bevy()
    }
}

impl TryFrom<&PyInfinitePlane3d> for InfinitePlane3d {
    type Error = PyErr;

    fn try_from(plane: &PyInfinitePlane3d) -> PyResult<Self> {
        plane.to_bevy()
    }
}
