use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    math::Vec2,
};
use pybevy_core::{PyResource, ResourceStorage, resource_initializer};
use pybevy_macros::pyresource;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::mouse_scroll_unit::PyMouseScrollUnit;

#[pyresource(AccumulatedMouseMotion, no_clone, bridge)]
#[pyclass(name = "AccumulatedMouseMotion", extends = PyResource)]
pub struct PyAccumulatedMouseMotion {
    pub(crate) storage: ResourceStorage<AccumulatedMouseMotion>,
}

#[pymethods]
impl PyAccumulatedMouseMotion {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        resource_initializer(Self {
            storage: ResourceStorage::owned(AccumulatedMouseMotion { delta: Vec2::ZERO }),
        })
    }

    #[getter]
    fn delta(&self) -> PyResult<PyVec2> {
        Ok(self.storage.snapshot_field_as(|motion| &motion.delta)?)
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(m) => format!(
                "AccumulatedMouseMotion(delta=Vec2({}, {}))",
                m.delta.x, m.delta.y
            ),
            Err(_) => "AccumulatedMouseMotion(<invalid>)".to_string(),
        }
    }
}

#[pyresource(AccumulatedMouseScroll, bridge)]
#[pyclass(name = "AccumulatedMouseScroll", extends = PyResource, eq, from_py_object)]
#[derive(Debug)]
pub struct PyAccumulatedMouseScroll {
    pub(crate) storage: ResourceStorage<AccumulatedMouseScroll>,
}

impl PartialEq for PyAccumulatedMouseScroll {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if left == right)
    }
}

impl PyAccumulatedMouseScroll {
    pub fn from_bevy(resource: &AccumulatedMouseScroll) -> PyClassInitializer<Self> {
        resource_initializer((*resource).into())
    }
}

#[pymethods]
impl PyAccumulatedMouseScroll {
    #[new]
    #[pyo3(signature = (unit = PyMouseScrollUnit::Line))]
    fn new(unit: PyMouseScrollUnit) -> PyClassInitializer<Self> {
        resource_initializer(
            AccumulatedMouseScroll {
                delta: Vec2::ZERO,
                unit: unit.into(),
            }
            .into(),
        )
    }

    #[getter]
    fn delta(&self) -> PyResult<PyVec2> {
        Ok(self.storage.snapshot_field_as(|scroll| &scroll.delta)?)
    }

    #[getter]
    fn unit(&self) -> PyResult<PyMouseScrollUnit> {
        Ok(self.as_ref()?.unit.into())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(scroll) => format!(
                "AccumulatedMouseScroll(delta=Vec2({}, {}), unit={:?})",
                scroll.delta.x, scroll.delta.y, scroll.unit
            ),
            Err(_) => "AccumulatedMouseScroll(<invalid>)".to_string(),
        }
    }
}
