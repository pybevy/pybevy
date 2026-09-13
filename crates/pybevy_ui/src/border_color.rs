use bevy::{color::Color, ui::BorderColor};
use pybevy_color::color::{IntoColorValue, PyColor};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(BorderColor, bridge)]
#[pyclass(name = "BorderColor", module = "pybevy.ui", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyBorderColor {
    pub(crate) storage: ComponentStorage<BorderColor>,
}

#[pymethods]
impl PyBorderColor {
    #[new]
    #[pyo3(signature = (color=None, *, top=None, right=None, bottom=None, left=None))]
    pub fn new(
        color: Option<IntoColorValue>,
        top: Option<PyColor>,
        right: Option<PyColor>,
        bottom: Option<PyColor>,
        left: Option<PyColor>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let base = color.map(|color| color.0).unwrap_or(Color::NONE);
        let bc = BorderColor {
            top: top.map(Color::try_from).transpose()?.unwrap_or(base),
            right: right.map(Color::try_from).transpose()?.unwrap_or(base),
            bottom: bottom.map(Color::try_from).transpose()?.unwrap_or(base),
            left: left.map(Color::try_from).transpose()?.unwrap_or(base),
        };
        Ok(Self::from_owned(bc).into())
    }

    #[staticmethod]
    pub fn all(py: Python<'_>, color: IntoColorValue) -> PyResult<Py<Self>> {
        let color = color.0;
        Py::new(py, Self::from_owned(BorderColor::all(color)))
    }

    #[getter]
    pub fn top(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |border| &border.top, py)
    }

    #[setter]
    pub fn set_top(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.top = color;
        Ok(())
    }

    #[getter]
    pub fn right(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |border| &border.right, py)
    }

    #[setter]
    pub fn set_right(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.right = color;
        Ok(())
    }

    #[getter]
    pub fn bottom(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |border| &border.bottom, py)
    }

    #[setter]
    pub fn set_bottom(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.bottom = color;
        Ok(())
    }

    #[getter]
    pub fn left(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |border| &border.left, py)
    }

    #[setter]
    pub fn set_left(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.left = color;
        Ok(())
    }

    pub fn set_all(
        mut slf: PyRefMut<'_, Self>,
        color: IntoColorValue,
    ) -> PyResult<PyRefMut<'_, Self>> {
        let color = color.0;
        slf.as_mut()?.set_all(color);
        Ok(slf)
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
