use std::f32::consts::FRAC_PI_3;

use bevy::{
    mesh::Meshable,
    shape::{CircularSector, Measured2d},
};
use pybevy_core::repr::float_repr;
use pybevy_macros::pyconstructor;
use pybevy_math::vec2::PyVec2;
use pybevy_shape::primitives::{
    PyArc2d,
    arc2d::{
        checked_arc, checked_arc_from_degrees, checked_arc_from_radians, checked_arc_from_turns,
    },
};
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCircularSectorMeshBuilder,
};

#[pyclass(name = "CircularSector", module = "pybevy.shape", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyCircularSector(pub(crate) CircularSector);

impl From<PyCircularSector> for CircularSector {
    fn from(py: PyCircularSector) -> Self {
        py.0
    }
}

impl From<CircularSector> for PyCircularSector {
    fn from(sector: CircularSector) -> Self {
        PyCircularSector(sector)
    }
}

#[pyconstructor("CircularSector", keyword_only(arc), conflicts((radius, half_angle), (arc)))]
#[pymethods]
impl PyCircularSector {
    #[new]
    pub fn new(
        #[default(0.5)] radius: f32,
        #[default(2.0 * FRAC_PI_3, text = "2.0943951023931953")] half_angle: f32,
        #[expected("Arc2d")] arc: Option<PyArc2d>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(a) = arc {
            return Ok((Self(CircularSector { arc: a.try_into()? }), PyMeshable).into());
        }
        Ok((
            Self(CircularSector {
                arc: checked_arc::<Self>(radius, half_angle)?,
            }),
            PyMeshable,
        )
            .into())
    }

    #[staticmethod]
    pub fn from_radians(py: Python<'_>, radius: f32, angle: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSector {
                    arc: checked_arc_from_radians::<Self>(radius, angle)?,
                }),
                PyMeshable,
            ),
        )
    }

    #[staticmethod]
    pub fn from_degrees(py: Python<'_>, radius: f32, angle: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSector {
                    arc: checked_arc_from_degrees::<Self>(radius, angle)?,
                }),
                PyMeshable,
            ),
        )
    }

    #[staticmethod]
    pub fn from_turns(py: Python<'_>, radius: f32, fraction: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSector {
                    arc: checked_arc_from_turns::<Self>(radius, fraction)?,
                }),
                PyMeshable,
            ),
        )
    }

    #[getter]
    pub fn arc(&self) -> PyArc2d {
        PyArc2d::from_read_only(&self.0.arc)
    }

    #[setter]
    pub fn set_arc(&mut self, value: PyArc2d) -> PyResult<()> {
        self.0.arc = value.try_into()?;
        Ok(())
    }

    pub fn radius(&self) -> f32 {
        self.0.radius()
    }

    pub fn half_angle(&self) -> f32 {
        self.0.half_angle()
    }

    pub fn angle(&self) -> f32 {
        self.0.angle()
    }

    pub fn arc_length(&self) -> f32 {
        self.0.arc_length()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn half_chord_length(&self) -> f32 {
        self.0.half_chord_length()
    }

    pub fn chord_length(&self) -> f32 {
        self.0.chord_length()
    }

    pub fn chord_midpoint(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.chord_midpoint())
    }

    pub fn apothem(&self) -> f32 {
        self.0.apothem()
    }

    pub fn sagitta(&self) -> f32 {
        self.0.sagitta()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCircularSectorMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "CircularSector(radius={}, angle={})",
            float_repr(self.0.radius()),
            float_repr(self.0.angle())
        )
    }
}
