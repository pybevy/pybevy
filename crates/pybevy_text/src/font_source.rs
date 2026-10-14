use bevy::{
    asset::Handle,
    text::{Font, FontSource, GenericFontFamily},
};
use pybevy_core::{PyAsset, PyHandle, extract_handle_from_any, repr::str_repr};
use pybevy_macros::pyenum;
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyList, PyTuple},
};

use crate::generic_font_family::PyGenericFontFamily;

#[pyenum(FontSource, manual)]
#[pyclass(
    name = "FontSource",
    module = "pybevy.text",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyFontSource {
    Handle { value: PyHandle },
    Family { value: String },
    Families { value: String },
    List { value: Vec<PyFontSource> },
    Generic { value: PyGenericFontFamily },
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
            FontSource::Families(source) => PyFontSource::Families {
                value: source.to_string(),
            },
            FontSource::List(sources) => PyFontSource::List {
                value: sources.iter().map(PyFontSource::from).collect(),
            },
            FontSource::Generic(generic) => PyFontSource::Generic {
                value: (*generic).into(),
            },
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
            PyFontSource::Families { value } => FontSource::Families(value.as_str().into()),
            PyFontSource::List { value } => FontSource::List(
                value
                    .iter()
                    .map(FontSource::try_from)
                    .collect::<PyResult<_>>()?,
            ),
            PyFontSource::Generic { value } => FontSource::Generic((*value).into()),
        })
    }
}

/// Accepted inputs at Bevy's `Into<FontSource>` boundaries.
#[derive(Debug, Clone)]
pub struct IntoFontSource(pub FontSource);

impl FromPyObject<'_, '_> for IntoFontSource {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        Ok(Self(extract_font_source_from_any(&obj)?))
    }
}

/// Extract a [`FontSource`], mirroring bevy's `From` impls: a `FontSource`, a font
/// `Handle`, a CSS family list (`From<&str>`), a `GenericFontFamily`, or a list or
/// tuple of `FontSource` (`From<Vec<FontSource>>`, `From<[FontSource; N]>`).
pub fn extract_font_source_from_any(obj: &Bound<'_, PyAny>) -> PyResult<FontSource> {
    if let Ok(source) = obj.extract::<PyFontSource>() {
        return (&source).try_into();
    }
    if obj.is_instance_of::<PyHandle>() || obj.is_instance_of::<PyAsset>() {
        let handle = extract_handle_from_any(obj)?;
        let handle: Handle<Font> = (&handle).try_into()?;
        return Ok(FontSource::Handle(handle));
    }
    if let Ok(families) = obj.extract::<String>() {
        return Ok(FontSource::Families(families.into()));
    }
    if let Ok(generic) = obj.extract::<PyGenericFontFamily>() {
        return Ok(FontSource::Generic(generic.into()));
    }
    if obj.is_instance_of::<PyList>() || obj.is_instance_of::<PyTuple>() {
        return obj
            .try_iter()?
            .map(|item| {
                let item = item?;
                let Ok(source) = item.extract::<PyFontSource>() else {
                    return Err(PyTypeError::new_err(format!(
                        "FontSource list items must be FontSource, got {}",
                        item.get_type().name()?
                    )));
                };
                FontSource::try_from(&source)
            })
            .collect::<PyResult<Vec<_>>>()
            .map(FontSource::List);
    }
    Err(PyTypeError::new_err(format!(
        "expected FontSource, Handle, str, GenericFontFamily, or a list of FontSource, got {}",
        obj.get_type().name()?
    )))
}

fn generic(family: GenericFontFamily) -> PyFontSource {
    PyFontSource::Generic {
        value: family.into(),
    }
}

#[pymethods]
impl PyFontSource {
    #[staticmethod]
    fn family(family: String) -> Self {
        PyFontSource::Family { value: family }
    }

    #[staticmethod]
    fn families(source: String) -> Self {
        PyFontSource::Families { value: source }
    }

    #[staticmethod]
    fn list(list: &Bound<'_, PyAny>) -> PyResult<Self> {
        let sources = list
            .try_iter()?
            .map(|item| Ok(PyFontSource::from(&extract_font_source_from_any(&item?)?)))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(PyFontSource::List { value: sources })
    }

    #[staticmethod]
    fn serif() -> Self {
        generic(GenericFontFamily::Serif)
    }

    #[staticmethod]
    fn sans_serif() -> Self {
        generic(GenericFontFamily::SansSerif)
    }

    #[staticmethod]
    fn cursive() -> Self {
        generic(GenericFontFamily::Cursive)
    }

    #[staticmethod]
    fn fantasy() -> Self {
        generic(GenericFontFamily::Fantasy)
    }

    #[staticmethod]
    fn monospace() -> Self {
        generic(GenericFontFamily::Monospace)
    }

    #[staticmethod]
    fn system_ui() -> Self {
        generic(GenericFontFamily::SystemUi)
    }

    #[staticmethod]
    fn ui_serif() -> Self {
        generic(GenericFontFamily::UiSerif)
    }

    #[staticmethod]
    fn ui_sans_serif() -> Self {
        generic(GenericFontFamily::UiSansSerif)
    }

    #[staticmethod]
    fn ui_monospace() -> Self {
        generic(GenericFontFamily::UiMonospace)
    }

    #[staticmethod]
    fn ui_rounded() -> Self {
        generic(GenericFontFamily::UiRounded)
    }

    #[staticmethod]
    fn emoji() -> Self {
        generic(GenericFontFamily::Emoji)
    }

    #[staticmethod]
    fn math() -> Self {
        generic(GenericFontFamily::Math)
    }

    #[staticmethod]
    fn fang_song() -> Self {
        generic(GenericFontFamily::FangSong)
    }

    fn flatten(&self) -> PyResult<Vec<PyFontSource>> {
        let source = FontSource::try_from(self)?;
        Ok(source
            .flatten()
            .into_iter()
            .map(PyFontSource::from)
            .collect())
    }

    fn is_list_like(&self) -> bool {
        matches!(
            self,
            PyFontSource::Families { .. } | PyFontSource::List { .. }
        )
    }

    fn is_recursive(&self) -> bool {
        match self {
            PyFontSource::List { value } => value.iter().any(PyFontSource::is_list_like),
            _ => false,
        }
    }

    fn __repr__(&self) -> String {
        match self {
            PyFontSource::Handle { .. } => "FontSource.Handle(...)".to_string(),
            PyFontSource::Family { value } => format!("FontSource.Family({})", str_repr(value)),
            PyFontSource::Families { value } => {
                format!("FontSource.Families({})", str_repr(value))
            }
            PyFontSource::List { value } => {
                let items: Vec<String> = value.iter().map(PyFontSource::__repr__).collect();
                format!("FontSource.List([{}])", items.join(", "))
            }
            PyFontSource::Generic { value } => {
                format!("FontSource.Generic({})", value.__repr__())
            }
        }
    }
}
