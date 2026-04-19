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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_path_only() {
        let ap = PyAssetPath::new("models/tree.glb".into(), None);
        assert_eq!(ap.path(), "models/tree.glb");
        assert_eq!(ap.label(), None);
        assert_eq!(ap.source(), None);
    }

    #[test]
    fn new_with_label() {
        let ap = PyAssetPath::new("models/tree.glb".into(), Some("Scene0".into()));
        assert_eq!(ap.path(), "models/tree.glb");
        assert_eq!(ap.label(), Some("Scene0".into()));
    }

    #[test]
    fn py_new_with_source() {
        let ap = PyAssetPath::py_new(
            "models/tree.glb".into(),
            Some("Scene0".into()),
            Some("embedded".into()),
        );
        assert_eq!(ap.path(), "models/tree.glb");
        assert_eq!(ap.label(), Some("Scene0".into()));
        assert_eq!(ap.source(), Some("embedded".into()));
    }

    #[test]
    fn to_bevy_asset_path_simple() {
        let ap = PyAssetPath::new("textures/wood.png".into(), None);
        let bevy_path: AssetPath<'static> = (&ap).into();
        assert_eq!(bevy_path.path().to_string_lossy(), "textures/wood.png");
        assert_eq!(bevy_path.label(), None);
    }

    #[test]
    fn to_bevy_asset_path_with_label() {
        let ap = PyAssetPath::new("scene.glb".into(), Some("Mesh0".into()));
        let bevy_path: AssetPath<'static> = (&ap).into();
        assert_eq!(bevy_path.label(), Some("Mesh0"));
    }

    #[test]
    fn round_trip_simple() {
        let original = PyAssetPath::new("a/b.png".into(), None);
        let bevy_path: AssetPath<'static> = (&original).into();
        let restored: PyAssetPath = bevy_path.into();
        assert_eq!(original, restored);
    }

    #[test]
    fn round_trip_with_label() {
        let original = PyAssetPath::new("scene.glb".into(), Some("MyLabel".into()));
        let bevy_path: AssetPath<'static> = (&original).into();
        let restored: PyAssetPath = bevy_path.into();
        assert_eq!(original, restored);
    }

    #[test]
    fn from_bevy_strips_default_source() {
        let bevy_path = AssetPath::from_path(Path::new("test.png"));
        let py_path: PyAssetPath = bevy_path.into();
        assert_eq!(py_path.source(), None);
    }

    #[test]
    fn equality() {
        let a = PyAssetPath::new("a.png".into(), None);
        let b = PyAssetPath::new("a.png".into(), None);
        let c = PyAssetPath::new("b.png".into(), None);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn equality_label_matters() {
        let a = PyAssetPath::new("a.glb".into(), Some("Scene0".into()));
        let b = PyAssetPath::new("a.glb".into(), Some("Scene1".into()));
        assert_ne!(a, b);
    }
}
