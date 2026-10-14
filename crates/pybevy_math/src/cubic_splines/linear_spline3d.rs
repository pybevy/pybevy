use bevy::{
    curve::cubic_splines::{CubicGenerator, CyclicCubicGenerator, LinearSpline},
    math::Vec3,
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve3d::PyCubicCurve3d;
use crate::vec3::PyVec3;

#[pyclass(
    name = "LinearSpline3d",
    module = "pybevy.curve",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyLinearSpline3d {
    spline: LinearSpline<Vec3>,
}

#[pymethods]
impl PyLinearSpline3d {
    #[new]
    pub fn new(points: Vec<PyVec3>) -> PyResult<Self> {
        let points: Vec<Vec3> = points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyLinearSpline3d {
            spline: LinearSpline::new(points),
        })
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve3d>> {
        let curve = self
            .spline
            .to_curve()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve3d::from_curve(curve))
    }

    pub fn to_curve_cyclic(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve3d>> {
        let curve = self
            .spline
            .to_curve_cyclic()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve3d::from_curve(curve))
    }

    fn __repr__(&self) -> String {
        format!("LinearSpline3d({} points)", self.spline.points.len())
    }
}
