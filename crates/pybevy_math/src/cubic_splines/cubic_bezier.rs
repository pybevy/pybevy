use bevy::math::{
    Vec2,
    cubic_splines::{CubicBezier, CubicGenerator},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve::PyCubicCurve2d;
use crate::vec2::PyVec2;

#[pyclass(name = "CubicBezier2d", frozen)]
#[derive(Debug, Clone)]
pub struct PyCubicBezier2d {
    bezier: CubicBezier<Vec2>,
}

#[pymethods]
impl PyCubicBezier2d {
    #[new]
    pub fn new(control_points: Vec<[PyVec2; 4]>) -> Self {
        let control_points: Vec<[Vec2; 4]> = control_points
            .into_iter()
            .map(|points| {
                [
                    points[0].clone().into(),
                    points[1].clone().into(),
                    points[2].clone().into(),
                    points[3].clone().into(),
                ]
            })
            .collect();

        PyCubicBezier2d {
            bezier: CubicBezier::new(control_points),
        }
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve2d>> {
        let curve = self
            .bezier
            .to_curve()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve2d::from_curve(curve))
    }

    fn __repr__(&self) -> String {
        format!(
            "CubicBezier2d({} segments)",
            self.bezier.control_points.len()
        )
    }
}
