use bevy::text::{Strikethrough, StrikethroughColor, Underline, UnderlineColor};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Strikethrough, unit, bridge)]
#[pyclass(name = "Strikethrough", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyStrikethrough;

impl From<Strikethrough> for PyStrikethrough {
    fn from(_: Strikethrough) -> Self {
        PyStrikethrough
    }
}

impl From<PyStrikethrough> for Strikethrough {
    fn from(_: PyStrikethrough) -> Self {
        Strikethrough
    }
}

impl TryFrom<&Strikethrough> for PyStrikethrough {
    type Error = PyErr;
    fn try_from(_: &Strikethrough) -> PyResult<Self> {
        Ok(PyStrikethrough)
    }
}

#[pymethods]
impl PyStrikethrough {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyStrikethrough, PyComponent)
    }

    fn __repr__(&self) -> &'static str {
        "Strikethrough"
    }
}

#[pycomponent(Underline, unit, bridge)]
#[pyclass(name = "Underline", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyUnderline;

impl From<Underline> for PyUnderline {
    fn from(_: Underline) -> Self {
        PyUnderline
    }
}

impl From<PyUnderline> for Underline {
    fn from(_: PyUnderline) -> Self {
        Underline
    }
}

impl TryFrom<&Underline> for PyUnderline {
    type Error = PyErr;
    fn try_from(_: &Underline) -> PyResult<Self> {
        Ok(PyUnderline)
    }
}

#[pymethods]
impl PyUnderline {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyUnderline, PyComponent)
    }

    fn __repr__(&self) -> &'static str {
        "Underline"
    }
}

#[pycomponent(StrikethroughColor, bridge, batch_only_fields = [0 as color])]
#[pyclass(name = "StrikethroughColor", extends = PyComponent)]
#[derive(Debug)]
pub struct PyStrikethroughColor {
    pub(crate) storage: ComponentStorage<StrikethroughColor>,
}

#[pymethods]
impl PyStrikethroughColor {
    #[new]
    #[pyo3(signature = (color = PyColor::default()))]
    pub fn new(color: PyColor) -> (Self, PyComponent) {
        Self::from_owned(StrikethroughColor(color.into()))
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.0 = color.into();
        Ok(())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}

#[pycomponent(UnderlineColor, bridge, batch_only_fields = [0 as color])]
#[pyclass(name = "UnderlineColor", extends = PyComponent)]
#[derive(Debug)]
pub struct PyUnderlineColor {
    pub(crate) storage: ComponentStorage<UnderlineColor>,
}

#[pymethods]
impl PyUnderlineColor {
    #[new]
    #[pyo3(signature = (color = PyColor::default()))]
    pub fn new(color: PyColor) -> (Self, PyComponent) {
        Self::from_owned(UnderlineColor(color.into()))
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.0 = color.into();
        Ok(())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
