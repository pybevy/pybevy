use bevy::{
    color::Color,
    ui::{BoxShadow, ShadowStyle},
};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent, ValueStorage, public_error, repr::object_repr};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyIndexError, prelude::*};

use crate::{shadow_style::PyShadowStyle, val::PyVal};

#[pycomponent(BoxShadow, bridge)]
#[pyclass(name = "BoxShadow", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyBoxShadow {
    pub(crate) storage: ComponentStorage<BoxShadow>,
}

fn shadow_index(index: isize, length: usize) -> PyResult<usize> {
    let normalized = if index >= 0 {
        usize::try_from(index).ok()
    } else {
        length.checked_sub(index.unsigned_abs())
    };
    normalized
        .filter(|position| *position < length)
        .ok_or_else(|| PyIndexError::new_err(public_error::UI_SHADOW_INDEX_OUT_OF_RANGE))
}

#[pymethods]
impl PyBoxShadow {
    #[new]
    #[pyo3(signature = (shadows = Vec::new()))]
    pub fn new(shadows: Vec<PyShadowStyle>) -> PyResult<PyClassInitializer<Self>> {
        let shadow_styles = shadows
            .into_iter()
            .map(ShadowStyle::try_from)
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self::from_owned(BoxShadow(shadow_styles)).into())
    }

    #[staticmethod]
    #[pyo3(name = "new")]
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

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.is_empty())
    }

    pub fn clear(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear();
        Ok(())
    }

    pub fn __getitem__(&self, index: isize) -> PyResult<PyShadowStyle> {
        let inner = self.as_ref()?;
        let index = shadow_index(index, inner.0.len())?;
        Ok(PyShadowStyle::from_borrowed(
            ValueStorage::read_only_snapshot(inner.0[index]),
        ))
    }

    pub fn __setitem__(&mut self, index: isize, style: PyShadowStyle) -> PyResult<()> {
        let style = style.try_into()?;
        let index = shadow_index(index, self.as_ref()?.0.len())?;
        self.as_mut()?.0[index] = style;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("BoxShadow({})", object_repr(self.shadows()?)))
    }
}
