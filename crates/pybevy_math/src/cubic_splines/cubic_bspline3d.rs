use bevy::math::{
    Vec3,
    cubic_splines::{CubicBSpline, CubicGenerator},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve3d::PyCubicCurve3d;
use crate::vec3::PyVec3;

#[pyclass(
    name = "CubicBSpline3d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicBSpline3d {
    spline: CubicBSpline<Vec3>,
}

#[pymethods]
impl PyCubicBSpline3d {
    #[new]
    pub fn new(control_points: Vec<PyVec3>) -> PyResult<Self> {
        let control_points: Vec<Vec3> = control_points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyCubicBSpline3d {
            spline: CubicBSpline::new(control_points),
        })
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve3d>> {
        let curve = self
            .spline
            .to_curve()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve3d::from_curve(curve))
    }

    fn __repr__(&self) -> String {
        format!(
            "CubicBSpline3d({} points)",
            self.spline.control_points.len()
        )
    }
}
