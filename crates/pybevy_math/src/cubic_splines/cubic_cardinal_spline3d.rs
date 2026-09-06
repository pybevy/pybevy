use bevy::math::{
    Vec3,
    cubic_splines::{CubicCardinalSpline, CubicGenerator},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve3d::PyCubicCurve3d;
use crate::vec3::PyVec3;

#[pyclass(
    name = "CubicCardinalSpline3d",
    module = "pybevy.math",
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicCardinalSpline3d {
    spline: CubicCardinalSpline<Vec3>,
}

#[pymethods]
impl PyCubicCardinalSpline3d {
    #[new]
    pub fn new(tension: f32, control_points: Vec<PyVec3>) -> PyResult<Self> {
        let control_points: Vec<Vec3> = control_points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyCubicCardinalSpline3d {
            spline: CubicCardinalSpline::new(tension, control_points),
        })
    }

    #[staticmethod]
    pub fn new_catmull_rom(control_points: Vec<PyVec3>) -> PyResult<Self> {
        let control_points: Vec<Vec3> = control_points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyCubicCardinalSpline3d {
            spline: CubicCardinalSpline::new_catmull_rom(control_points),
        })
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve3d>> {
        let curve = self
            .spline
            .to_curve()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve3d::from_curve(curve))
    }

    #[getter]
    pub fn tension(&self) -> f32 {
        self.spline.tension
    }

    #[setter]
    pub fn set_tension(&mut self, value: f32) {
        self.spline.tension = value;
    }

    fn __repr__(&self) -> String {
        format!(
            "CubicCardinalSpline3d(tension={}, {} points)",
            self.spline.tension,
            self.spline.control_points.len()
        )
    }
}
