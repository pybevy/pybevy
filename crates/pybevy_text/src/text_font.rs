use bevy::{
    asset::Handle,
    text::{Font, FontSmoothing, FontWeight, TextFont},
};
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any, pycomponent::ComponentStorage};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::{
    font_features::PyFontFeatures, font_smoothing::PyFontSmoothing, font_weight::PyFontWeight,
};

#[pycomponent(TextFont, bridge, view_fields = [font_size])]
#[pyclass(name = "TextFont", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyTextFont {
    pub(crate) storage: ComponentStorage<TextFont>,
}

impl PyTextFont {
    fn default_font_size() -> f32 {
        TextFont::default().font_size
    }

    fn default_font_smoothing() -> PyFontSmoothing {
        TextFont::default().font_smoothing.into()
    }

    fn default_weight() -> PyFontWeight {
        FontWeight::NORMAL.into()
    }

    fn default_font_features() -> PyFontFeatures {
        PyFontFeatures::default()
    }
}

#[pymethods]
impl PyTextFont {
    #[new]
    #[pyo3(signature = (
        font = None,
        font_size = Self::default_font_size(),
        font_smoothing = Self::default_font_smoothing(),
        weight = Self::default_weight(),
        font_features = Self::default_font_features()
    ))]
    pub fn new(
        font: Option<&Bound<'_, PyAny>>,
        font_size: f32,
        font_smoothing: PyFontSmoothing,
        weight: PyFontWeight,
        font_features: PyFontFeatures,
    ) -> PyResult<(Self, PyComponent)> {
        let font_handle: Handle<Font> = match font {
            Some(handle_obj) => {
                let handle = extract_handle_from_any(handle_obj)?;
                (&handle).try_into()?
            }
            None => Default::default(),
        };

        Ok(Self::from_owned(TextFont {
            font: font_handle,
            font_size,
            font_smoothing: font_smoothing.into(),
            weight: weight.into(),
            font_features: font_features.into(),
        }))
    }

    #[staticmethod]
    pub fn from_font_size(py: Python<'_>, font_size: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(TextFont {
                font: Default::default(),
                font_size,
                font_smoothing: FontSmoothing::AntiAliased,
                ..Default::default()
            }),
        )
    }

    #[getter]
    pub fn font(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(&self.as_ref()?.font))
    }

    #[setter]
    pub fn set_font(&mut self, font: &Bound<'_, PyAny>) -> PyResult<()> {
        let handle = extract_handle_from_any(font)?;
        self.as_mut()?.font = (&handle).try_into()?;
        Ok(())
    }

    #[getter]
    pub fn font_size(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.font_size)
    }

    #[setter]
    pub fn set_font_size(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.font_size = value;
        Ok(())
    }

    #[getter]
    pub fn font_smoothing(&self) -> PyResult<PyFontSmoothing> {
        Ok(self.as_ref()?.font_smoothing.into())
    }

    #[setter]
    pub fn set_font_smoothing(&mut self, font_smoothing: PyFontSmoothing) -> PyResult<()> {
        self.as_mut()?.font_smoothing = font_smoothing.into();
        Ok(())
    }

    #[getter]
    pub fn weight(&self) -> PyResult<PyFontWeight> {
        Ok(self.as_ref()?.weight.into())
    }

    #[setter]
    pub fn set_weight(&mut self, weight: PyFontWeight) -> PyResult<()> {
        self.as_mut()?.weight = weight.into();
        Ok(())
    }

    #[getter]
    pub fn font_features(&self) -> PyResult<PyFontFeatures> {
        Ok(self.as_ref()?.font_features.clone().into())
    }

    #[setter]
    pub fn set_font_features(&mut self, font_features: PyFontFeatures) -> PyResult<()> {
        self.as_mut()?.font_features = font_features.into();
        Ok(())
    }

    #[pyo3(name = "with_font")]
    pub fn with_font(slf: Py<Self>, py: Python<'_>, font: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        let handle = extract_handle_from_any(font)?;
        slf.borrow_mut(py).as_mut()?.font = (&handle).try_into()?;
        Ok(slf)
    }

    #[pyo3(name = "with_font_size")]
    pub fn with_font_size(slf: Py<Self>, py: Python<'_>, font_size: f32) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.font_size = font_size;
        Ok(slf)
    }

    #[pyo3(name = "with_font_smoothing")]
    pub fn with_font_smoothing(
        slf: Py<Self>,
        py: Python<'_>,
        font_smoothing: PyFontSmoothing,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.font_smoothing = font_smoothing.into();
        Ok(slf)
    }

    #[pyo3(name = "with_weight")]
    pub fn with_weight(slf: Py<Self>, py: Python<'_>, weight: PyFontWeight) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.weight = weight.into();
        Ok(slf)
    }

    #[pyo3(name = "with_font_features")]
    pub fn with_font_features(
        slf: Py<Self>,
        py: Python<'_>,
        font_features: PyFontFeatures,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.font_features = font_features.into();
        Ok(slf)
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
