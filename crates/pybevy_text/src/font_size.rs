use bevy::text::FontSize;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyclass(name = "FontSize", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyFontSize {
    Px { value: f32 },
    Vw { value: f32 },
    Vh { value: f32 },
    VMin { value: f32 },
    VMax { value: f32 },
    Rem { value: f32 },
}

impl From<FontSize> for PyFontSize {
    fn from(size: FontSize) -> Self {
        match size {
            FontSize::Px(value) => PyFontSize::Px { value },
            FontSize::Vw(value) => PyFontSize::Vw { value },
            FontSize::Vh(value) => PyFontSize::Vh { value },
            FontSize::VMin(value) => PyFontSize::VMin { value },
            FontSize::VMax(value) => PyFontSize::VMax { value },
            FontSize::Rem(value) => PyFontSize::Rem { value },
        }
    }
}

impl From<PyFontSize> for FontSize {
    fn from(size: PyFontSize) -> Self {
        match size {
            PyFontSize::Px { value } => FontSize::Px(value),
            PyFontSize::Vw { value } => FontSize::Vw(value),
            PyFontSize::Vh { value } => FontSize::Vh(value),
            PyFontSize::VMin { value } => FontSize::VMin(value),
            PyFontSize::VMax { value } => FontSize::VMax(value),
            PyFontSize::Rem { value } => FontSize::Rem(value),
        }
    }
}

/// Extract a [`FontSize`], mirroring bevy's `impl Into<FontSize>` parameters:
/// a plain float is `FontSize::Px` (bevy's `From<f32>`), otherwise a `FontSize`.
pub fn extract_font_size_from_any(obj: &Bound<'_, PyAny>) -> PyResult<FontSize> {
    if let Ok(size) = obj.extract::<PyFontSize>() {
        return Ok(size.into());
    }
    if let Ok(value) = obj.extract::<f32>() {
        return Ok(FontSize::Px(value));
    }
    Err(PyTypeError::new_err(format!(
        "expected a float (pixels) or FontSize, got {}",
        obj.get_type().name()?
    )))
}

#[pymethods]
impl PyFontSize {
    fn __repr__(&self) -> String {
        match self {
            PyFontSize::Px { value } => format!("FontSize.Px({value})"),
            PyFontSize::Vw { value } => format!("FontSize.Vw({value})"),
            PyFontSize::Vh { value } => format!("FontSize.Vh({value})"),
            PyFontSize::VMin { value } => format!("FontSize.VMin({value})"),
            PyFontSize::VMax { value } => format!("FontSize.VMax({value})"),
            PyFontSize::Rem { value } => format!("FontSize.Rem({value})"),
        }
    }
}
