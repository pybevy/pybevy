use bevy::{color::Color, ui::BorderColor};
use pybevy_color::color::{IntoColorValue, PyColor};
use pybevy_core::{ComponentStorage, Omittable, PyComponent, repr::object_repr};
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
    #[pyo3(signature = (
        color=IntoColorValue(Color::NONE),
        *,
        top=Omittable::default(),
        right=Omittable::default(),
        bottom=Omittable::default(),
        left=Omittable::default(),
    ))]
    pub fn new(
        color: IntoColorValue,
        top: Omittable<PyRef<'_, PyColor>>,
        right: Omittable<PyRef<'_, PyColor>>,
        bottom: Omittable<PyRef<'_, PyColor>>,
        left: Omittable<PyRef<'_, PyColor>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let base = color.0;
        let side = |side: Omittable<PyRef<'_, PyColor>>| -> PyResult<Color> {
            side.into_option()
                .map_or(Ok(base), |color| Color::try_from(&*color))
        };
        let bc = BorderColor {
            top: side(top)?,
            right: side(right)?,
            bottom: side(bottom)?,
            left: side(left)?,
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
    pub fn set_top(&mut self, top: IntoColorValue) -> PyResult<()> {
        self.as_mut()?.top = top.0;
        Ok(())
    }

    #[getter]
    pub fn right(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |border| &border.right, py)
    }

    #[setter]
    pub fn set_right(&mut self, right: IntoColorValue) -> PyResult<()> {
        self.as_mut()?.right = right.0;
        Ok(())
    }

    #[getter]
    pub fn bottom(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |border| &border.bottom, py)
    }

    #[setter]
    pub fn set_bottom(&mut self, bottom: IntoColorValue) -> PyResult<()> {
        self.as_mut()?.bottom = bottom.0;
        Ok(())
    }

    #[getter]
    pub fn left(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |border| &border.left, py)
    }

    #[setter]
    pub fn set_left(&mut self, left: IntoColorValue) -> PyResult<()> {
        self.as_mut()?.left = left.0;
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
            "BorderColor(top={}, right={}, bottom={}, left={})",
            object_repr(PyColor::from(bc.top)),
            object_repr(PyColor::from(bc.right)),
            object_repr(PyColor::from(bc.bottom)),
            object_repr(PyColor::from(bc.left))
        ))
    }
}
