use bevy::math::{
    Vec2,
    cubic_splines::{CubicNurbs, RationalGenerator},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::rational_curve2d::PyRationalCurve2d;
use crate::vec2::PyVec2;

#[pyclass(
    name = "CubicNurbs2d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicNurbs2d {
    nurbs: CubicNurbs<Vec2>,
}

#[pymethods]
impl PyCubicNurbs2d {
    #[new]
    #[pyo3(signature = (control_points, weights = None, knots = None))]
    pub fn new(
        control_points: Vec<PyVec2>,
        weights: Option<Vec<f32>>,
        knots: Option<Vec<f32>>,
    ) -> PyResult<Self> {
        let control_points: Vec<Vec2> = control_points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        let nurbs = CubicNurbs::new(control_points, weights, knots)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        Ok(PyCubicNurbs2d { nurbs })
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyRationalCurve2d>> {
        let curve = self
            .nurbs
            .to_curve()
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        Py::new(py, PyRationalCurve2d::from_curve(curve))
    }

    #[getter]
    pub fn control_points(&self) -> Vec<PyVec2> {
        self.nurbs
            .control_points
            .iter()
            .map(|p| (*p).into())
            .collect()
    }

    #[getter]
    pub fn weights(&self) -> Vec<f32> {
        self.nurbs.weights.clone()
    }

    #[getter]
    pub fn knots(&self) -> Vec<f32> {
        self.nurbs.knots.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "CubicNurbs2d({} control points)",
            self.nurbs.control_points.len()
        )
    }
}
