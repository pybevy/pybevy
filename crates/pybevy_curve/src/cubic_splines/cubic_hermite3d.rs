use bevy::{
    curve::cubic_splines::{CubicGenerator, CubicHermite, CyclicCubicGenerator},
    math::Vec3,
};
use pybevy_core::public_error;
use pybevy_math::vec3::PyVec3;
use pyo3::{exceptions::PyValueError, prelude::*};

use super::cubic_curve3d::PyCubicCurve3d;

#[pyclass(
    name = "CubicHermite3d",
    module = "pybevy.curve",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicHermite3d {
    hermite: CubicHermite<Vec3>,
}

#[pymethods]
impl PyCubicHermite3d {
    #[new]
    pub fn new(control_points: Vec<PyVec3>, tangents: Vec<PyVec3>) -> PyResult<Self> {
        let control_points: Vec<Vec3> = control_points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        let tangents: Vec<Vec3> = tangents
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        if control_points.len() != tangents.len() {
            return Err(PyValueError::new_err(
                public_error::cubic_hermite_tangent_count(control_points.len(), tangents.len()),
            ));
        }

        Ok(PyCubicHermite3d {
            hermite: CubicHermite::new(control_points, tangents),
        })
    }

    pub fn to_curve(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve3d>> {
        let curve = self
            .hermite
            .to_curve()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve3d::from_curve(curve))
    }

    pub fn to_curve_cyclic(&self, py: Python<'_>) -> PyResult<Py<PyCubicCurve3d>> {
        let curve = self
            .hermite
            .to_curve_cyclic()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Py::new(py, PyCubicCurve3d::from_curve(curve))
    }

    fn __repr__(&self) -> String {
        format!(
            "CubicHermite3d({} points)",
            self.hermite.control_points.len()
        )
    }
}
