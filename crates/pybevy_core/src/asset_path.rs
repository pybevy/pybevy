use std::path::Path;

use bevy::asset::AssetPath;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pyclass(name = "AssetPath", eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAssetPath {
    path: String,
    label: Option<String>,
    source: Option<String>,
}

impl PyAssetPath {
    pub fn new(path: String, label: Option<String>) -> Self {
        Self {
            path,
            label,
            source: None,
        }
    }
}

#[pymethods]
impl PyAssetPath {
    #[getter]
    pub fn label(&self) -> Option<String> {
        self.label.clone()
    }

    #[getter]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    #[getter]
    pub fn source(&self) -> Option<String> {
        self.source.clone()
    }

    #[staticmethod]
    pub fn parse(asset_path: &str) -> PyResult<Self> {
        match AssetPath::try_parse(asset_path) {
            Ok(asset_path) => Ok(asset_path.into()),
            Err(err) => Err(PyValueError::new_err(err.to_string())),
        }
    }

    #[new]
    #[pyo3(signature = (path, label=None, source=None))]
    pub fn py_new(path: String, label: Option<String>, source: Option<String>) -> Self {
        Self {
            path,
            label,
            source,
        }
    }
}

impl From<&PyAssetPath> for AssetPath<'static> {
    fn from(value: &PyAssetPath) -> Self {
        // Build path string with source if provided
        let path_with_source = if let Some(source) = &value.source {
            format!("{}://{}", source, value.path)
        } else {
            value.path.clone()
        };

        let path = AssetPath::from_path(Path::new(&path_with_source));
        match &value.label {
            Some(label) => path.with_label(label),
            None => path,
        }
        .into_owned()
    }
}

impl<'a> From<AssetPath<'a>> for PyAssetPath {
    fn from(asset_path: AssetPath<'a>) -> Self {
        let source_str = asset_path.source().as_str();

        // Only include source if it's not None and not the default source
        let source = match source_str {
            Some(s) if s != "default" => Some(s.to_string()),
            _ => None,
        };

        PyAssetPath {
            path: asset_path.path().to_string_lossy().to_string(),
            label: asset_path.label().map(|l| l.to_string()),
            source,
        }
    }
}
