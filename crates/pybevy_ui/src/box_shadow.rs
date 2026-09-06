use bevy::{
    color::Color,
    ui::{BoxShadow, ShadowStyle},
};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent, ValueStorage};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyIndexError, prelude::*};

use crate::{shadow_style::PyShadowStyle, val::PyVal};

#[pycomponent(BoxShadow, bridge)]
#[pyclass(name = "BoxShadow", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyBoxShadow {
    pub(crate) storage: ComponentStorage<BoxShadow>,
}

#[pymethods]
impl PyBoxShadow {
    #[new]
    #[pyo3(signature = (shadows = None))]
    pub fn new(shadows: Option<Vec<PyShadowStyle>>) -> PyResult<PyClassInitializer<Self>> {
        let shadow_styles = shadows
            .map(|styles| {
                styles
                    .into_iter()
                    .map(ShadowStyle::try_from)
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self::from_owned(BoxShadow(shadow_styles)).into())
    }

    #[staticmethod]
    pub fn single(
        py: Python,
        color: PyColor,
        x_offset: PyVal,
        y_offset: PyVal,
        spread_radius: PyVal,
        blur_radius: PyVal,
    ) -> PyResult<Py<Self>> {
        let bevy_color = Color::try_from(color)?;
        let (instance, base) = Self::from_owned(BoxShadow::new(
            bevy_color,
            x_offset.into(),
            y_offset.into(),
            spread_radius.into(),
            blur_radius.into(),
        ));
        Py::new(py, (instance, base))
    }

    #[getter]
    pub fn shadows(&self) -> PyResult<Vec<PyShadowStyle>> {
        Ok(self
            .as_ref()?
            .0
            .iter()
            .map(|style| PyShadowStyle::from_borrowed(ValueStorage::read_only_snapshot(*style)))
            .collect())
    }

    #[setter]
    pub fn set_shadows(&mut self, shadows: Vec<PyShadowStyle>) -> PyResult<()> {
        let shadows = shadows
            .into_iter()
            .map(ShadowStyle::try_from)
            .collect::<PyResult<Vec<_>>>()?;
        self.as_mut()?.0 = shadows;
        Ok(())
    }

    pub fn push(&mut self, style: PyShadowStyle) -> PyResult<()> {
        let style = style.try_into()?;
        self.as_mut()?.0.push(style);
        Ok(())
    }

    pub fn pop(&mut self) -> PyResult<Option<PyShadowStyle>> {
        Ok(self.as_mut()?.0.pop().map(PyShadowStyle::from))
    }

    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.0.len())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.is_empty())
    }

    pub fn clear(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear();
        Ok(())
    }

    pub fn __getitem__(&self, index: usize) -> PyResult<PyShadowStyle> {
        let inner = self.as_ref()?;
        if index >= inner.0.len() {
            return Err(PyIndexError::new_err("shadow index out of range"));
        }
        Ok(PyShadowStyle::from_borrowed(
            ValueStorage::read_only_snapshot(inner.0[index]),
        ))
    }

    pub fn __setitem__(&mut self, index: usize, style: PyShadowStyle) -> PyResult<()> {
        let style = style.try_into()?;
        let mut inner = self.as_mut()?;
        if index >= inner.0.len() {
            return Err(PyIndexError::new_err("shadow index out of range"));
        }
        inner.0[index] = style;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let len = self.as_ref()?.0.len();
        Ok(format!("BoxShadow({} shadow(s))", len))
    }
}
