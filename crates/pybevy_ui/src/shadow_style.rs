use bevy::{color::Color, ui::ShadowStyle};
use pybevy_color::color::PyColor;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyvalue]
#[pyclass(name = "ShadowStyle", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyShadowStyle {
    pub(crate) storage: ValueStorage<ShadowStyle>,
}

impl PartialEq for PyShadowStyle {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl Default for PyShadowStyle {
    fn default() -> Self {
        Self::from_owned(ShadowStyle::default())
    }
}

impl From<ShadowStyle> for PyShadowStyle {
    fn from(style: ShadowStyle) -> Self {
        Self::from_owned(style)
    }
}

impl TryFrom<PyShadowStyle> for ShadowStyle {
    type Error = PyErr;

    fn try_from(py_style: PyShadowStyle) -> PyResult<Self> {
        py_style.to_bevy()
    }
}

impl TryFrom<&PyShadowStyle> for ShadowStyle {
    type Error = PyErr;

    fn try_from(py_style: &PyShadowStyle) -> PyResult<Self> {
        py_style.to_bevy()
    }
}

#[pymethods]
impl PyShadowStyle {
    #[new]
    #[pyo3(signature = (
        color = None,
        x_offset = PyVal::percent_unchecked(20.0),
        y_offset = PyVal::percent_unchecked(20.0),
        spread_radius = PyVal::zero(),
        blur_radius = PyVal::percent_unchecked(10.0)
    ))]
    pub fn new(
        color: Option<PyColor>,
        x_offset: PyVal,
        y_offset: PyVal,
        spread_radius: PyVal,
        blur_radius: PyVal,
    ) -> PyResult<Self> {
        let bevy_color = color
            .map(Color::try_from)
            .transpose()?
            .unwrap_or(Color::BLACK);
        Ok(Self::from_owned(ShadowStyle {
            color: bevy_color,
            x_offset: x_offset.into(),
            y_offset: y_offset.into(),
            spread_radius: spread_radius.into(),
            blur_radius: blur_radius.into(),
        }))
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_storage(self.storage.borrow_field(|style| &style.color)?, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let bevy_color = Color::try_from(color)?;
        self.as_mut()?.color = bevy_color;
        Ok(())
    }

    #[getter]
    pub fn x_offset(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.x_offset.into())
    }

    #[setter]
    pub fn set_x_offset(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.x_offset = value.into();
        Ok(())
    }

    #[getter]
    pub fn y_offset(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.y_offset.into())
    }

    #[setter]
    pub fn set_y_offset(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.y_offset = value.into();
        Ok(())
    }

    #[getter]
    pub fn spread_radius(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.spread_radius.into())
    }

    #[setter]
    pub fn set_spread_radius(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.spread_radius = value.into();
        Ok(())
    }

    #[getter]
    pub fn blur_radius(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.blur_radius.into())
    }

    #[setter]
    pub fn set_blur_radius(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.blur_radius = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let style = self.as_ref()?;
        Ok(format!(
            "ShadowStyle(color={:?}, x_offset={:?}, y_offset={:?}, spread_radius={:?}, blur_radius={:?})",
            style.color, style.x_offset, style.y_offset, style.spread_radius, style.blur_radius
        ))
    }
}
