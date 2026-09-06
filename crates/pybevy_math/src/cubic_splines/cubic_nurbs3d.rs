use bevy::math::{
    Vec3,
    cubic_splines::{CubicNurbs, RationalGenerator},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::rational_curve3d::PyRationalCurve3d;
use crate::vec3::PyVec3;

#[pyclass(
    name = "CubicNurbs3d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicNurbs3d {
    nurbs: CubicNurbs<Vec3>,
}

#[pymethods]
impl PyCubicNurbs3d {
    #[new]
    #[pyo3(signature = (control_points, weights = None, knots = None))]
    pub fn new(
        control_points: Vec<PyVec3>,
        weights: Option<Vec<f32>>,
        knots: Option<Vec<f32>>,
    ) -> PyResult<Self> {
        let control_points: Vec<Vec3> = control_points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        let nurbs = CubicNurbs::new(control_points, weights, knots)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        Ok(PyCubicNurbs3d { nurbs })
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyRationalCurve3d>> {
        let curve = self
            .nurbs
            .to_curve()
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        Py::new(py, PyRationalCurve3d::from_curve(curve))
    }

    #[getter]
    pub fn control_points(&self) -> Vec<PyVec3> {
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
            "CubicNurbs3d({} control points)",
            self.nurbs.control_points.len()
        )
    }
}
