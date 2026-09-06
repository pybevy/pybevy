use bevy::text::{Strikethrough, StrikethroughColor, Underline, UnderlineColor};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Strikethrough, unit, bridge)]
#[pyclass(name = "Strikethrough", module = "pybevy.text", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyStrikethrough, PyComponent).into()
    }

    fn __repr__(&self) -> &'static str {
        "Strikethrough"
    }
}

#[pycomponent(Underline, unit, bridge)]
#[pyclass(name = "Underline", module = "pybevy.text", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyUnderline, PyComponent).into()
    }

    fn __repr__(&self) -> &'static str {
        "Underline"
    }
}

#[pycomponent(StrikethroughColor, bridge, batch_only_fields = [0 as color])]
#[pyclass(name = "StrikethroughColor", module = "pybevy.text", extends = PyComponent)]
#[derive(Debug)]
pub struct PyStrikethroughColor {
    pub(crate) storage: ComponentStorage<StrikethroughColor>,
}

#[pymethods]
impl PyStrikethroughColor {
    #[new]
    #[pyo3(signature = (color = PyColor::default()))]
    pub fn new(color: PyColor) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(StrikethroughColor(color.try_into()?)).into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |color| &color.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.0 = color;
        Ok(())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}

#[pycomponent(UnderlineColor, bridge, batch_only_fields = [0 as color])]
#[pyclass(name = "UnderlineColor", module = "pybevy.text", extends = PyComponent)]
#[derive(Debug)]
pub struct PyUnderlineColor {
    pub(crate) storage: ComponentStorage<UnderlineColor>,
}

#[pymethods]
impl PyUnderlineColor {
    #[new]
    #[pyo3(signature = (color = PyColor::default()))]
    pub fn new(color: PyColor) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(UnderlineColor(color.try_into()?)).into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |color| &color.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.0 = color;
        Ok(())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
