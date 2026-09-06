use bevy::{math::BVec2, ui::IgnoreScroll};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(IgnoreScroll, bridge)]
#[pyclass(name = "IgnoreScroll", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyIgnoreScroll {
    pub(crate) storage: ComponentStorage<IgnoreScroll>,
}

impl PyIgnoreScroll {
    fn default_ignore_x() -> bool {
        IgnoreScroll::default().0.x
    }

    fn default_ignore_y() -> bool {
        IgnoreScroll::default().0.y
    }
}

#[pymethods]
impl PyIgnoreScroll {
    #[new]
    #[pyo3(signature = (ignore_x = Self::default_ignore_x(), ignore_y = Self::default_ignore_y()))]
    pub fn new(ignore_x: bool, ignore_y: bool) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(IgnoreScroll(BVec2 {
            x: ignore_x,
            y: ignore_y,
        }))
        .into())
    }

    #[getter]
    pub fn ignore_x(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.x)
    }

    #[setter]
    pub fn set_ignore_x(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0.x = value;
        Ok(())
    }

    #[getter]
    pub fn ignore_y(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.y)
    }

    #[setter]
    pub fn set_ignore_y(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0.y = value;
        Ok(())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()?.0 == other.as_ref()?.0)
    }
}
