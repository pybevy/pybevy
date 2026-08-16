use bevy::math::{CompassOctant, CompassQuadrant};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::vec2::PyVec2;

#[pyenum(CompassOctant)]
#[pyclass(name = "CompassOctant", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyCompassOctant {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

#[pymethods]
impl PyCompassOctant {
    #[staticmethod]
    pub fn from_index(index: usize) -> Option<Self> {
        CompassOctant::from_index(index).map(|c| c.into())
    }

    pub fn to_index(&self) -> usize {
        let octant: CompassOctant = (*self).into();
        octant.to_index()
    }

    pub fn opposite(&self) -> Self {
        let octant: CompassOctant = (*self).into();
        octant.opposite().into()
    }

    pub fn is_in_direction(&self, origin: PyVec2, candidate: PyVec2) -> PyResult<bool> {
        let octant: CompassOctant = (*self).into();
        Ok(octant.is_in_direction(origin.try_into()?, candidate.try_into()?))
    }

    pub fn __neg__(&self) -> Self {
        self.opposite()
    }
}

#[pyenum(CompassQuadrant)]
#[pyclass(name = "CompassQuadrant", eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyCompassQuadrant {
    North,
    East,
    South,
    West,
}

#[pymethods]
impl PyCompassQuadrant {
    #[staticmethod]
    pub fn from_index(index: usize) -> Option<Self> {
        CompassQuadrant::from_index(index).map(|c| c.into())
    }

    pub fn to_index(&self) -> usize {
        let quadrant: CompassQuadrant = (*self).into();
        quadrant.to_index()
    }

    pub fn opposite(&self) -> Self {
        let quadrant: CompassQuadrant = (*self).into();
        quadrant.opposite().into()
    }

    pub fn __neg__(&self) -> Self {
        self.opposite()
    }
}
