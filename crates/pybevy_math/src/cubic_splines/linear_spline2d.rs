use bevy::math::{
    Vec2,
    cubic_splines::{CubicGenerator, LinearSpline},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve::PyCubicCurve2d;
use crate::vec2::PyVec2;

#[pyclass(
    name = "LinearSpline2d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyLinearSpline2d {
    spline: LinearSpline<Vec2>,
}

#[pymethods]
impl PyLinearSpline2d {
    #[new]
    pub fn new(points: Vec<PyVec2>) -> PyResult<Self> {
        let points: Vec<Vec2> = points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyLinearSpline2d {
            spline: LinearSpline::new(points),
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
        format!("LinearSpline2d({} points)", self.spline.points.len())
    }
}
