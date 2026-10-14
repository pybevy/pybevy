use bevy::{
    mesh::Meshable,
    shape::{Measured2d, RegularPolygon},
};
use pybevy_core::{
    domain::{IntegerInput, check_for},
    repr::float_repr,
};
use pybevy_math::primitives::dimension::dimension;
use pyo3::prelude::*;

use super::circle::PyCircle;
use crate::{
    mesh_builder::PyMeshBuilder,
    meshable::PyMeshable,
    primitives::{PyRegularPolygonMeshBuilder, validation::AT_LEAST_THREE},
};

#[pyclass(name = "RegularPolygon", module = "pybevy.shape", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyRegularPolygon(pub(crate) RegularPolygon);

impl From<PyRegularPolygon> for RegularPolygon {
    fn from(py_polygon: PyRegularPolygon) -> Self {
        py_polygon.0
    }
}

impl From<RegularPolygon> for PyRegularPolygon {
    fn from(polygon: RegularPolygon) -> Self {
        PyRegularPolygon(polygon)
    }
}

#[pymethods]
impl PyRegularPolygon {
    #[new]
    #[pyo3(signature = (circumradius = 0.5, sides = IntegerInput::new(6)))]
    pub fn new(circumradius: f32, sides: IntegerInput<u32>) -> PyResult<PyClassInitializer<Self>> {
        let circumradius = dimension::<Self, _>("circumradius", circumradius)?;
        let sides = check_for::<Self, _>("sides", sides, AT_LEAST_THREE)?.stored();
        Ok((Self(RegularPolygon::new(circumradius, sides)), PyMeshable).into())
    }

    #[getter]
    pub fn circumcircle(&self, py: Python<'_>) -> PyResult<Py<PyCircle>> {
        Py::new(
            py,
            (PyCircle::from_read_only(&self.0.circumcircle), PyMeshable),
        )
    }

    #[setter]
    pub fn set_circumcircle(&mut self, value: &PyCircle) -> PyResult<()> {
        let mut circle = value.try_get()?;
        circle.radius = dimension::<Self, _>("circumradius", circle.radius)?;
        self.0.circumcircle = circle;
        Ok(())
    }

    #[getter]
    pub fn sides(&self) -> u32 {
        self.0.sides
    }

    #[setter]
    pub fn set_sides(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        self.0.sides = check_for::<Self, _>("sides", value, AT_LEAST_THREE)?.stored();
        Ok(())
    }

    pub fn circumradius(&self) -> f32 {
        self.0.circumradius()
    }

    pub fn inradius(&self) -> f32 {
        self.0.inradius()
    }

    pub fn side_length(&self) -> f32 {
        self.0.side_length()
    }

    pub fn internal_angle_degrees(&self) -> f32 {
        self.0.internal_angle_degrees()
    }

    pub fn internal_angle_radians(&self) -> f32 {
        self.0.internal_angle_radians()
    }

    pub fn external_angle_degrees(&self) -> f32 {
        self.0.external_angle_degrees()
    }

    pub fn external_angle_radians(&self) -> f32 {
        self.0.external_angle_radians()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyRegularPolygonMeshBuilder>> {
        Py::new(
            py,
            (
                PyRegularPolygonMeshBuilder::new(self.0.mesh(), self.0.sides),
                PyMeshBuilder,
            ),
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "RegularPolygon(circumradius={}, sides={})",
            float_repr(self.0.circumradius()),
            self.0.sides
        )
    }
}
