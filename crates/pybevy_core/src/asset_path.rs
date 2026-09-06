use std::path::Path;

use bevy::asset::AssetPath;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pyclass(
    name = "AssetPath",
    module = "pybevy.assets",
    eq,
    frozen,
    from_py_object
)]
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
        let path = AssetPath::from_path(Path::new(&value.path)).into_owned();
        let path = match &value.source {
            Some(source) => path.with_source(source.clone()),
            None => path,
        };
        match &value.label {
            Some(label) => path.with_label(label.clone()),
            None => path,
        }
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
    fn to_bevy_asset_path_sets_the_source_field() {
        // The source belongs in `AssetPath::source`, not as a `source://`
        // prefix inside the path: an AssetServer resolves only the former.
        let ap = PyAssetPath::py_new("shaders/x.wgsl".into(), None, Some("embedded".into()));
        let bevy_path: AssetPath<'static> = (&ap).into();
        assert_eq!(bevy_path.source().as_str(), Some("embedded"));
        assert_eq!(bevy_path.path().to_string_lossy(), "shaders/x.wgsl");
    }

    #[test]
    fn round_trip_with_source() {
        let original = PyAssetPath::py_new("shaders/x.wgsl".into(), None, Some("embedded".into()));
        let bevy_path: AssetPath<'static> = (&original).into();
        let restored: PyAssetPath = bevy_path.into();
        assert_eq!(original, restored);
    }

    #[test]
    fn round_trip_with_source_and_label() {
        let original = PyAssetPath::py_new(
            "scene.glb".into(),
            Some("Mesh0".into()),
            Some("embedded".into()),
        );
        let bevy_path: AssetPath<'static> = (&original).into();
        let restored: PyAssetPath = bevy_path.into();
        assert_eq!(original, restored);
    }

    #[test]
    fn parsed_source_survives_conversion_to_bevy() {
        // `AssetPath.parse` splits the source out; converting back must not
        // fold it into the path again.
        let parsed = PyAssetPath::parse("embedded://shaders/x.wgsl").unwrap();
        assert_eq!(parsed.source(), Some("embedded".into()));

        let bevy_path: AssetPath<'static> = (&parsed).into();
        assert_eq!(bevy_path, AssetPath::parse("embedded://shaders/x.wgsl"));
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
