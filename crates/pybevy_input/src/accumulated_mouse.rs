use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    math::Vec2,
};
use pybevy_core::PyResource;
use pybevy_math::PyVec2;
use pyo3::prelude::*;

use crate::mouse_scroll_unit::PyMouseScrollUnit;

#[pyclass(name = "AccumulatedMouseMotion", extends = PyResource, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAccumulatedMouseMotion {
    pub delta: PyVec2,
}

impl PyAccumulatedMouseMotion {
    pub fn from_bevy(resource: &AccumulatedMouseMotion) -> (Self, PyResource) {
        (
            PyAccumulatedMouseMotion {
                delta: resource.delta.into(),
            },
            PyResource,
        )
    }
}

#[pymethods]
impl PyAccumulatedMouseMotion {
    #[new]
    fn new() -> (Self, PyResource) {
        (
            PyAccumulatedMouseMotion {
                delta: Vec2::ZERO.into(),
            },
            PyResource,
        )
    }

    #[getter]
    fn delta(&self) -> PyVec2 {
        self.delta.clone()
    }

    fn __repr__(&self) -> String {
        let d = self.delta.get();
        format!("AccumulatedMouseMotion(delta=Vec2({}, {}))", d.x, d.y)
    }
}

#[pyclass(name = "AccumulatedMouseScroll", extends = PyResource, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAccumulatedMouseScroll {
    pub delta: PyVec2,
    pub unit: PyMouseScrollUnit,
}

impl PyAccumulatedMouseScroll {
    pub fn from_bevy(resource: &AccumulatedMouseScroll) -> (Self, PyResource) {
        (
            PyAccumulatedMouseScroll {
                delta: resource.delta.into(),
                unit: resource.unit.into(),
            },
            PyResource,
        )
    }
}

#[pymethods]
impl PyAccumulatedMouseScroll {
    #[new]
    #[pyo3(signature = (unit = PyMouseScrollUnit::Line))]
    fn new(unit: PyMouseScrollUnit) -> (Self, PyResource) {
        (
            PyAccumulatedMouseScroll {
                delta: Vec2::ZERO.into(),
                unit,
            },
            PyResource,
        )
    }

    #[getter]
    fn delta(&self) -> PyVec2 {
        self.delta.clone()
    }

    #[getter]
    fn unit(&self) -> PyMouseScrollUnit {
        self.unit
    }

    fn __repr__(&self) -> String {
        let d = self.delta.get();
        format!(
            "AccumulatedMouseScroll(delta=Vec2({}, {}), unit={:?})",
            d.x, d.y, self.unit
        )
    }
}
