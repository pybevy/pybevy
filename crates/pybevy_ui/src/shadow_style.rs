use bevy::{color::Color, ui::ShadowStyle};
use pybevy_color::color::PyColor;
use pyo3::prelude::*;

use crate::val::PyVal;

#[pyclass(name = "ShadowStyle", eq)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PyShadowStyle {
    pub inner: ShadowStyle,
}

impl From<ShadowStyle> for PyShadowStyle {
    fn from(style: ShadowStyle) -> Self {
        PyShadowStyle { inner: style }
    }
}

impl From<PyShadowStyle> for ShadowStyle {
    fn from(py_style: PyShadowStyle) -> Self {
        py_style.inner
    }
}

impl From<&PyShadowStyle> for ShadowStyle {
    fn from(py_style: &PyShadowStyle) -> Self {
        py_style.inner
    }
}

#[pymethods]
impl PyShadowStyle {
    #[new]
    #[pyo3(signature = (
        color = None,
        x_offset = PyVal::zero(),
        y_offset = PyVal::zero(),
        spread_radius = PyVal::zero(),
        blur_radius = PyVal::zero()
    ))]
    pub fn new(
        color: Option<PyColor>,
        x_offset: PyVal,
        y_offset: PyVal,
        spread_radius: PyVal,
        blur_radius: PyVal,
    ) -> Self {
        let bevy_color: Color = color.map(|c| c.into()).unwrap_or(Color::NONE);
        PyShadowStyle {
            inner: ShadowStyle {
                color: bevy_color,
                x_offset: x_offset.into(),
                y_offset: y_offset.into(),
                spread_radius: spread_radius.into(),
                blur_radius: blur_radius.into(),
            },
        }
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.inner.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) {
        let bevy_color: Color = color.into();
        self.inner.color = bevy_color;
    }

    #[getter]
    pub fn x_offset(&self) -> PyVal {
        self.inner.x_offset.into()
    }

    #[setter]
    pub fn set_x_offset(&mut self, value: PyVal) {
        self.inner.x_offset = value.into();
    }

    #[getter]
    pub fn y_offset(&self) -> PyVal {
        self.inner.y_offset.into()
    }

    #[setter]
    pub fn set_y_offset(&mut self, value: PyVal) {
        self.inner.y_offset = value.into();
    }

    #[getter]
    pub fn spread_radius(&self) -> PyVal {
        self.inner.spread_radius.into()
    }

    #[setter]
    pub fn set_spread_radius(&mut self, value: PyVal) {
        self.inner.spread_radius = value.into();
    }

    #[getter]
    pub fn blur_radius(&self) -> PyVal {
        self.inner.blur_radius.into()
    }

    #[setter]
    pub fn set_blur_radius(&mut self, value: PyVal) {
        self.inner.blur_radius = value.into();
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ShadowStyle(color={:?}, x_offset={:?}, y_offset={:?}, spread_radius={:?}, blur_radius={:?})",
            self.inner.color,
            self.inner.x_offset,
            self.inner.y_offset,
            self.inner.spread_radius,
            self.inner.blur_radius
        )
    }
}
