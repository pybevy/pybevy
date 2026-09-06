use bevy::math::{
    Vec2,
    cubic_splines::{CubicBSpline, CubicGenerator},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve::PyCubicCurve2d;
use crate::vec2::PyVec2;

#[pyclass(
    name = "CubicBSpline2d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicBSpline2d {
    spline: CubicBSpline<Vec2>,
}

#[pymethods]
impl PyCubicBSpline2d {
    #[new]
    pub fn new(control_points: Vec<PyVec2>) -> PyResult<Self> {
        let control_points: Vec<Vec2> = control_points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyCubicBSpline2d {
            spline: CubicBSpline::new(control_points),
        })
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve2d>> {
        let curve = self
            .spline
            .to_curve()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve2d::from_curve(curve))
    }

    fn __repr__(&self) -> String {
        format!(
            "CubicBSpline2d({} points)",
            self.spline.control_points.len()
        )
    }
}
