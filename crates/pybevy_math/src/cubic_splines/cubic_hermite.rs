use bevy::math::{
    Vec2,
    cubic_splines::{CubicGenerator, CubicHermite},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve::PyCubicCurve2d;
use crate::vec2::PyVec2;

#[pyclass(name = "CubicHermite2d", frozen)]
#[derive(Debug, Clone)]
pub struct PyCubicHermite2d {
    hermite: CubicHermite<Vec2>,
}

#[pymethods]
impl PyCubicHermite2d {
    #[new]
    pub fn new(control_points: Vec<PyVec2>, tangents: Vec<PyVec2>) -> Self {
        let control_points: Vec<Vec2> = control_points.into_iter().map(|p| p.into()).collect();

        let tangents: Vec<Vec2> = tangents.into_iter().map(|p| p.into()).collect();

        PyCubicHermite2d {
            hermite: CubicHermite::new(control_points, tangents),
        }
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve2d>> {
        let curve = self
            .hermite
            .to_curve()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve2d::from_curve(curve))
    }

    fn __repr__(&self) -> String {
        format!(
            "CubicHermite2d({} points)",
            self.hermite.control_points.len()
        )
    }
}
