use bevy::{color::Color, ui::BorderColor};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(BorderColor, bridge)]
#[pyclass(name = "BorderColor", extends = PyComponent, eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyBorderColor {
    pub(crate) storage: ComponentStorage<BorderColor>,
}

#[pymethods]
impl PyBorderColor {
    #[new]
    #[pyo3(signature = (color=None, *, top=None, right=None, bottom=None, left=None))]
    pub fn new(
        color: Option<PyColor>,
        top: Option<PyColor>,
        right: Option<PyColor>,
        bottom: Option<PyColor>,
        left: Option<PyColor>,
    ) -> (Self, PyComponent) {
        let base = color.map(Color::from).unwrap_or(Color::NONE);
        let bc = BorderColor {
            top: top.map(Color::from).unwrap_or(base),
            right: right.map(Color::from).unwrap_or(base),
            bottom: bottom.map(Color::from).unwrap_or(base),
            left: left.map(Color::from).unwrap_or(base),
        };
        Self::from_owned(bc)
    }

    #[staticmethod]
    pub fn all(py: Python<'_>, color: PyColor) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(BorderColor::all(Color::from(color))))
    }

    #[getter]
    pub fn top(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.top, py)
    }

    #[setter]
    pub fn set_top(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.top = color.into();
        Ok(())
    }

    #[getter]
    pub fn right(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.right, py)
    }

    #[setter]
    pub fn set_right(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.right = color.into();
        Ok(())
    }

    #[getter]
    pub fn bottom(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.bottom, py)
    }

    #[setter]
    pub fn set_bottom(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.bottom = color.into();
        Ok(())
    }

    #[getter]
    pub fn left(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.left, py)
    }

    #[setter]
    pub fn set_left(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.left = color.into();
        Ok(())
    }

    pub fn set_all(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.set_all(Color::from(color));
        Ok(())
    }

    pub fn is_fully_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_fully_transparent())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let bc = self.as_ref()?;
        Ok(format!(
            "BorderColor(top={:?}, right={:?}, bottom={:?}, left={:?})",
            bc.top, bc.right, bc.bottom, bc.left
        ))
    }
}
