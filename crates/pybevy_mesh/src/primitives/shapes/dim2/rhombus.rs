use bevy::{
    math::{Vec2, prelude::Measured2d, primitives::Rhombus},
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, StorageError, ValueStorage, repr::ReprLiteral};
use pybevy_macros::{pyconstructor, pyvalue};
use pybevy_math::{primitives::dimension::dimension, vec2::PyVec2};
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyRhombusMeshBuilder};

#[pyvalue]
#[pyclass(name = "Rhombus", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
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

fn check_half_diagonals(value: &Vec2) -> Result<(), StorageError> {
    Ok(dimension::<PyRhombus, _>("half_diagonals", *value).map(drop)?)
}

#[pyconstructor(
    "Rhombus",
    keyword_only(half_diagonals),
    conflicts((horizontal_diagonal, vertical_diagonal), (half_diagonals)),
)]
#[pymethods]
impl PyRhombus {
    #[new]
    pub fn new(
        #[default(1.0)] horizontal_diagonal: f32,
        #[default(1.0)] vertical_diagonal: f32,
        #[expected("Vec2")] half_diagonals: Option<PyVec2>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(hd) = half_diagonals {
            let half_diagonals = dimension::<Self, _>("half_diagonals", hd.try_into()?)?;
            return Ok((Self::from_owned(Rhombus { half_diagonals }), PyMeshable).into());
        }
        let rhombus = Rhombus::new(
            dimension::<Self, _>("horizontal_diagonal", horizontal_diagonal)?,
            dimension::<Self, _>("vertical_diagonal", vertical_diagonal)?,
        );
        Ok((Self::from_owned(rhombus), PyMeshable).into())
    }

    #[staticmethod]
    pub fn from_side(py: Python, side: f32) -> PyResult<Py<Self>> {
        let rhombus = Rhombus::from_side(dimension::<Self, _>("side", side)?);
        Py::new(py, (Self::from_owned(rhombus), PyMeshable))
    }

    #[staticmethod]
    pub fn from_inradius(py: Python, inradius: f32) -> PyResult<Py<Self>> {
        let rhombus = Rhombus::from_inradius(dimension::<Self, _>("inradius", inradius)?);
        check_half_diagonals(&rhombus.half_diagonals)?;
        Py::new(py, (Self::from_owned(rhombus), PyMeshable))
    }

    #[getter]
    pub fn half_diagonals(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |s| &s.half_diagonals,
            |s| &mut s.half_diagonals,
            check_half_diagonals,
        )?)
    }

    #[setter]
    pub fn set_half_diagonals(&mut self, value: PyVec2) -> PyResult<()> {
        let half_diagonals = value.try_into()?;
        let mut rhombus = self.as_mut()?;
        rhombus.half_diagonals = dimension::<Self, _>("half_diagonals", half_diagonals)?;
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
            "Rhombus(half_diagonals={})",
            self.as_ref()?.half_diagonals.repr_literal()
        ))
    }
}
