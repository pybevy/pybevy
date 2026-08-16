use bevy::{
    math::{prelude::Measured2d, primitives::Rhombus},
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyRhombusMeshBuilder};

#[pyvalue]
#[pyclass(name = "Rhombus", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRhombus {
    pub(crate) storage: ValueStorage<Rhombus>,
}

impl PartialEq for PyRhombus {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl TryFrom<PyRhombus> for Rhombus {
    type Error = PyErr;

    fn try_from(value: PyRhombus) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyRhombus> for Rhombus {
    type Error = PyErr;

    fn try_from(value: &PyRhombus) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl From<Rhombus> for PyRhombus {
    fn from(value: Rhombus) -> Self {
        Self::from_owned(value)
    }
}

#[pymethods]
impl PyRhombus {
    #[new]
    #[pyo3(signature = (horizontal_diagonal = 1.0, vertical_diagonal = 1.0, *, half_diagonals = None))]
    pub fn new(
        horizontal_diagonal: f32,
        vertical_diagonal: f32,
        half_diagonals: Option<PyVec2>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(hd) = half_diagonals {
            return Ok((
                Self::from_owned(Rhombus {
                    half_diagonals: hd.try_into()?,
                }),
                PyMeshable,
            )
                .into());
        }
        Ok((
            Self::from_owned(Rhombus::new(horizontal_diagonal, vertical_diagonal)),
            PyMeshable,
        )
            .into())
    }

    #[staticmethod]
    pub fn from_side(py: Python, side: f32) -> PyResult<Py<Self>> {
        Py::new(py, (Self::from_owned(Rhombus::from_side(side)), PyMeshable))
    }

    #[staticmethod]
    pub fn from_inradius(py: Python, inradius: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self::from_owned(Rhombus::from_inradius(inradius)),
                PyMeshable,
            ),
        )
    }

    #[getter]
    pub fn half_diagonals(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.half_diagonals)?)
    }

    #[setter]
    pub fn set_half_diagonals(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.half_diagonals = value.try_into()?;
        Ok(())
    }

    pub fn side(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.side())
    }

    pub fn circumradius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.circumradius())
    }

    pub fn inradius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.inradius())
    }

    pub fn area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.area())
    }

    pub fn perimeter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.perimeter())
    }

    pub fn closest_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.as_ref()?.closest_point(point.try_into()?),
        ))
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyRhombusMeshBuilder>> {
        Py::new(py, (self.as_ref()?.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Rhombus(half_diagonals=Vec2({}, {}))",
            self.as_ref()?.half_diagonals.x,
            self.as_ref()?.half_diagonals.y
        ))
    }
}
