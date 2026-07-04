use bevy::{
    asset::Handle,
    text::{Font, FontSource, FontStyle, FontWeight, FontWidth, TextFont},
};
use pybevy_core::{PyComponent, extract_handle_from_any, pycomponent::ComponentStorage};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::{
    font_features::PyFontFeatures,
    font_size::{PyFontSize, extract_font_size_from_any},
    font_smoothing::PyFontSmoothing,
    font_source::{PyFontSource, extract_font_source_from_any},
    font_style::PyFontStyle,
    font_weight::PyFontWeight,
    font_width::PyFontWidth,
};

#[pycomponent(TextFont, bridge)]
#[pyclass(name = "TextFont", extends = PyComponent)]
#[derive(Debug)]
pub struct PyTextFont {
    pub(crate) storage: ComponentStorage<TextFont>,
}

impl PyTextFont {
    fn default_font_smoothing() -> PyFontSmoothing {
        TextFont::default().font_smoothing.into()
    }

    fn default_weight() -> PyFontWeight {
        FontWeight::NORMAL.into()
    }

    fn default_width() -> PyFontWidth {
        FontWidth::default().into()
    }

    fn default_style() -> PyFontStyle {
        FontStyle::default().into()
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
        font_size = None,
        font_smoothing = Self::default_font_smoothing(),
        weight = Self::default_weight(),
        width = Self::default_width(),
        style = Self::default_style(),
        font_features = Self::default_font_features()
    ))]
    pub fn new(
        font: Option<&Bound<'_, PyAny>>,
        font_size: Option<&Bound<'_, PyAny>>,
        font_smoothing: PyFontSmoothing,
        weight: PyFontWeight,
        width: PyFontWidth,
        style: PyFontStyle,
        font_features: PyFontFeatures,
    ) -> PyResult<PyClassInitializer<Self>> {
        let font = match font {
            Some(obj) => extract_font_source_from_any(obj)?,
            None => FontSource::default(),
        };
        let font_size = match font_size {
            Some(obj) => extract_font_size_from_any(obj)?,
            None => TextFont::default().font_size,
        };

        Ok(Self::from_owned(TextFont {
            font,
            font_size,
            font_smoothing: font_smoothing.into(),
            weight: weight.into(),
            width: width.into(),
            style: style.into(),
            font_features: font_features.into(),
            ..Default::default()
        }).into())
    }

    #[staticmethod]
    pub fn from_font_size(py: Python<'_>, font_size: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(TextFont::from_font_size(extract_font_size_from_any(
                font_size,
            )?)),
        )
    }

    #[getter]
    pub fn font(&self) -> PyResult<PyFontSource> {
        Ok((&self.as_ref()?.font).into())
    }

    #[setter]
    pub fn set_font(&mut self, font: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.font = extract_font_source_from_any(font)?;
        Ok(())
    }

    #[getter]
    pub fn font_size(&self) -> PyResult<PyFontSize> {
        Ok(self.as_ref()?.font_size.into())
    }

    #[setter]
    pub fn set_font_size(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.font_size = extract_font_size_from_any(value)?;
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
    pub fn width(&self) -> PyResult<PyFontWidth> {
        Ok(self.as_ref()?.width.into())
    }

    #[setter]
    pub fn set_width(&mut self, width: PyFontWidth) -> PyResult<()> {
        self.as_mut()?.width = width.into();
        Ok(())
    }

    #[getter]
    pub fn style(&self) -> PyResult<PyFontStyle> {
        Ok(self.as_ref()?.style.into())
    }

    #[setter]
    pub fn set_style(&mut self, style: PyFontStyle) -> PyResult<()> {
        self.as_mut()?.style = style.into();
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
        let bevy_handle: Handle<Font> = (&handle).try_into()?;
        slf.borrow_mut(py).as_mut()?.font = FontSource::Handle(bevy_handle);
        Ok(slf)
    }

    #[pyo3(name = "with_family")]
    pub fn with_family(slf: Py<Self>, py: Python<'_>, family: String) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.font = FontSource::Family(family.into());
        Ok(slf)
    }

    #[pyo3(name = "with_font_size")]
    pub fn with_font_size(
        slf: Py<Self>,
        py: Python<'_>,
        font_size: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).as_mut()?.font_size = extract_font_size_from_any(font_size)?;
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
