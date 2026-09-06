use bevy::math::{
    Vec3,
    cubic_splines::{CubicBezier, CubicGenerator},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve3d::PyCubicCurve3d;
use crate::vec3::PyVec3;

#[pyclass(
    name = "CubicBezier3d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicBezier3d {
    bezier: CubicBezier<Vec3>,
}

#[pymethods]
impl PyCubicBezier3d {
    #[new]
    pub fn new(control_points: Vec<[PyVec3; 4]>) -> PyResult<Self> {
        let control_points: Vec<[Vec3; 4]> = control_points
            .into_iter()
            .map(|points| {
                Ok([
                    points[0].clone().try_into()?,
                    points[1].clone().try_into()?,
                    points[2].clone().try_into()?,
                    points[3].clone().try_into()?,
                ])
            })
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyCubicBezier3d {
            bezier: CubicBezier::new(control_points),
        })
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve3d>> {
        let curve = self
            .bezier
            .to_curve()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve3d::from_curve(curve))
    }

    fn __repr__(&self) -> String {
        format!(
            "CubicBezier3d({} segments)",
            self.bezier.control_points.len()
        )
    }
}
