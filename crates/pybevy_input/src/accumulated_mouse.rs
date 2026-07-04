use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    math::Vec2,
};
use pybevy_core::{PyResource, ResourceStorage};
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
        (
            Self {
                storage: ResourceStorage::owned(AccumulatedMouseMotion { delta: Vec2::ZERO }),
            },
            PyResource,
        )
            .into()
    }

    #[getter]
    fn delta(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.delta.into())
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

#[pyclass(name = "AccumulatedMouseScroll", extends = PyResource, eq, skip_from_py_object)]
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
    fn new(unit: PyMouseScrollUnit) -> PyClassInitializer<Self> {
        (
            PyAccumulatedMouseScroll {
                delta: Vec2::ZERO.into(),
                unit,
            },
            PyResource,
        )
            .into()
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
