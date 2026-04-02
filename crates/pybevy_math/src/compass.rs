use bevy::math::{CompassOctant, CompassQuadrant};
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

use crate::vec2::PyVec2;

#[bevy_enum(CompassOctant, from_only)]
#[pyclass(name = "CompassOctant", eq)]
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

    pub fn is_in_direction(&self, origin: PyVec2, candidate: PyVec2) -> bool {
        let octant: CompassOctant = (*self).into();
        octant.is_in_direction(origin.into(), candidate.into())
    }

    pub fn __neg__(&self) -> Self {
        self.opposite()
    }

    fn __repr__(&self) -> String {
        let name = match self {
            Self::North => "North",
            Self::NorthEast => "NorthEast",
            Self::East => "East",
            Self::SouthEast => "SouthEast",
            Self::South => "South",
            Self::SouthWest => "SouthWest",
            Self::West => "West",
            Self::NorthWest => "NorthWest",
        };
        format!("CompassOctant.{name}")
    }
}

#[bevy_enum(CompassQuadrant, from_only)]
#[pyclass(name = "CompassQuadrant", eq)]
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

    fn __repr__(&self) -> String {
        let name = match self {
            Self::North => "North",
            Self::East => "East",
            Self::South => "South",
            Self::West => "West",
        };
        format!("CompassQuadrant.{name}")
    }
}
