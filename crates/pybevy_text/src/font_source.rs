use bevy::{
    asset::Handle,
    text::{Font, FontSource},
};
use pybevy_core::{PyHandle, extract_handle_from_any};
use pyo3::prelude::*;

#[pyclass(name = "FontSource", frozen, eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyFontSource {
    Handle { value: PyHandle },
    Family { value: String },
    Serif(),
    SansSerif(),
    Cursive(),
    Fantasy(),
    Monospace(),
    SystemUi(),
    UiSerif(),
    UiSansSerif(),
    UiMonospace(),
    UiRounded(),
    Emoji(),
    Math(),
    FangSong(),
}

impl From<&FontSource> for PyFontSource {
    fn from(source: &FontSource) -> Self {
        match source {
            FontSource::Handle(handle) => PyFontSource::Handle {
                value: PyHandle::from(handle),
            },
            FontSource::Family(name) => PyFontSource::Family {
                value: name.to_string(),
            },
            FontSource::Serif => PyFontSource::Serif(),
            FontSource::SansSerif => PyFontSource::SansSerif(),
            FontSource::Cursive => PyFontSource::Cursive(),
            FontSource::Fantasy => PyFontSource::Fantasy(),
            FontSource::Monospace => PyFontSource::Monospace(),
            FontSource::SystemUi => PyFontSource::SystemUi(),
            FontSource::UiSerif => PyFontSource::UiSerif(),
            FontSource::UiSansSerif => PyFontSource::UiSansSerif(),
            FontSource::UiMonospace => PyFontSource::UiMonospace(),
            FontSource::UiRounded => PyFontSource::UiRounded(),
            FontSource::Emoji => PyFontSource::Emoji(),
            FontSource::Math => PyFontSource::Math(),
            FontSource::FangSong => PyFontSource::FangSong(),
        }
    }
}

impl TryFrom<&PyFontSource> for FontSource {
    type Error = PyErr;

    fn try_from(source: &PyFontSource) -> PyResult<Self> {
        Ok(match source {
            PyFontSource::Handle { value } => {
                let handle: Handle<Font> = value.try_into()?;
                FontSource::Handle(handle)
            }
            PyFontSource::Family { value } => FontSource::Family(value.as_str().into()),
            PyFontSource::Serif() => FontSource::Serif,
            PyFontSource::SansSerif() => FontSource::SansSerif,
            PyFontSource::Cursive() => FontSource::Cursive,
            PyFontSource::Fantasy() => FontSource::Fantasy,
            PyFontSource::Monospace() => FontSource::Monospace,
            PyFontSource::SystemUi() => FontSource::SystemUi,
            PyFontSource::UiSerif() => FontSource::UiSerif,
            PyFontSource::UiSansSerif() => FontSource::UiSansSerif,
            PyFontSource::UiMonospace() => FontSource::UiMonospace,
            PyFontSource::UiRounded() => FontSource::UiRounded,
            PyFontSource::Emoji() => FontSource::Emoji,
            PyFontSource::Math() => FontSource::Math,
            PyFontSource::FangSong() => FontSource::FangSong,
        })
    }
}

/// Extract a [`FontSource`], mirroring bevy's `From` impls: a `FontSource`,
/// a font `Handle` (`From<Handle<Font>>`), or a family name (`From<&str>`).
pub fn extract_font_source_from_any(obj: &Bound<'_, PyAny>) -> PyResult<FontSource> {
    if let Ok(source) = obj.extract::<PyFontSource>() {
        return (&source).try_into();
    }
    if let Ok(family) = obj.extract::<String>() {
        return Ok(FontSource::Family(family.into()));
    }
    let handle = extract_handle_from_any(obj)?;
    let handle: Handle<Font> = (&handle).try_into()?;
    Ok(FontSource::Handle(handle))
}

#[pymethods]
impl PyFontSource {
    fn __repr__(&self) -> String {
        match self {
            PyFontSource::Handle { .. } => "FontSource.Handle(...)".to_string(),
            PyFontSource::Family { value } => format!("FontSource.Family({value:?})"),
            PyFontSource::Serif() => "FontSource.Serif()".to_string(),
            PyFontSource::SansSerif() => "FontSource.SansSerif()".to_string(),
            PyFontSource::Cursive() => "FontSource.Cursive()".to_string(),
            PyFontSource::Fantasy() => "FontSource.Fantasy()".to_string(),
            PyFontSource::Monospace() => "FontSource.Monospace()".to_string(),
            PyFontSource::SystemUi() => "FontSource.SystemUi()".to_string(),
            PyFontSource::UiSerif() => "FontSource.UiSerif()".to_string(),
            PyFontSource::UiSansSerif() => "FontSource.UiSansSerif()".to_string(),
            PyFontSource::UiMonospace() => "FontSource.UiMonospace()".to_string(),
            PyFontSource::UiRounded() => "FontSource.UiRounded()".to_string(),
            PyFontSource::Emoji() => "FontSource.Emoji()".to_string(),
            PyFontSource::Math() => "FontSource.Math()".to_string(),
            PyFontSource::FangSong() => "FontSource.FangSong()".to_string(),
        }
    }
}
