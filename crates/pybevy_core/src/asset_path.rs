use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::Path,
};

use bevy::asset::AssetPath;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyString};

#[pyclass(
    name = "AssetPath",
    module = "pybevy.assets",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Hash)]
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

    /// Bevy's canonical `source://path#label` form of this path.
    fn display(&self) -> String {
        AssetPath::from(self).to_string()
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
    #[pyo3(signature = (*, source=None, path, label=None))]
    pub fn py_new(source: Option<String>, path: String, label: Option<String>) -> Self {
        Self {
            path,
            label,
            source,
        }
    }

    pub fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    pub fn __str__(&self) -> String {
        self.display()
    }

    pub fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let quoted = |value: Option<&str>| -> PyResult<String> {
            match value {
                None => Ok("None".to_string()),
                Some(value) => Ok(PyString::new(py, value).repr()?.to_str()?.to_owned()),
            }
        };
        Ok(format!(
            "AssetPath(source={}, path={}, label={})",
            quoted(self.source.as_deref())?,
            quoted(Some(self.path.as_str()))?,
            quoted(self.label.as_deref())?
        ))
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
            Some("embedded".into()),
            "models/tree.glb".into(),
            Some("Scene0".into()),
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
        let ap = PyAssetPath::py_new(Some("embedded".into()), "shaders/x.wgsl".into(), None);
        let bevy_path: AssetPath<'static> = (&ap).into();
        assert_eq!(bevy_path.source().as_str(), Some("embedded"));
        assert_eq!(bevy_path.path().to_string_lossy(), "shaders/x.wgsl");
    }

    #[test]
    fn round_trip_with_source() {
        let original = PyAssetPath::py_new(Some("embedded".into()), "shaders/x.wgsl".into(), None);
        let bevy_path: AssetPath<'static> = (&original).into();
        let restored: PyAssetPath = bevy_path.into();
        assert_eq!(original, restored);
    }

    #[test]
    fn round_trip_with_source_and_label() {
        let original = PyAssetPath::py_new(
            Some("embedded".into()),
            "scene.glb".into(),
            Some("Mesh0".into()),
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

    #[test]
    fn equal_values_produce_equal_hashes() {
        use std::hash::{Hash, Hasher};

        let pairs = [
            (
                PyAssetPath::new("a.png".into(), None),
                PyAssetPath::new("a.png".into(), None),
            ),
            (
                PyAssetPath::new("scene.glb".into(), Some("Mesh0".into())),
                PyAssetPath::new("scene.glb".into(), Some("Mesh0".into())),
            ),
            (
                PyAssetPath::py_new(Some("remote".into()), "models/cube.glb".into(), None),
                PyAssetPath::py_new(Some("remote".into()), "models/cube.glb".into(), None),
            ),
            (
                PyAssetPath::py_new(
                    Some("remote".into()),
                    "models/cube.glb".into(),
                    Some("Cube".into()),
                ),
                PyAssetPath::py_new(
                    Some("remote".into()),
                    "models/cube.glb".into(),
                    Some("Cube".into()),
                ),
            ),
        ];
        for (left, right) in pairs {
            assert_eq!(left, right);
            let mut left_hasher = DefaultHasher::new();
            let mut right_hasher = DefaultHasher::new();
            left.hash(&mut left_hasher);
            right.hash(&mut right_hasher);
            assert_eq!(left_hasher.finish(), right_hasher.finish());
        }
    }

    #[test]
    fn display_uses_the_bevy_asset_path_form() {
        assert_eq!(PyAssetPath::new("a.png".into(), None).display(), "a.png");
        assert_eq!(
            PyAssetPath::new("scene.glb".into(), Some("Mesh0".into())).display(),
            "scene.glb#Mesh0"
        );
        assert_eq!(
            PyAssetPath::py_new(Some("embedded".into()), "shaders/x.wgsl".into(), None).display(),
            "embedded://shaders/x.wgsl"
        );
        assert_eq!(
            PyAssetPath::py_new(
                Some("remote".into()),
                "models/cube.glb".into(),
                Some("Cube".into())
            )
            .display(),
            "remote://models/cube.glb#Cube"
        );
    }

    #[test]
    fn display_keeps_non_ascii_paths_and_labels() {
        let ap = PyAssetPath::py_new(
            Some("remote".into()),
            "textures/ü-ñ.png".into(),
            Some("héllo".into()),
        );
        assert_eq!(ap.display(), "remote://textures/ü-ñ.png#héllo");
    }

    #[test]
    fn display_round_trips_through_parse() {
        let parsed = PyAssetPath::parse("remote://models/cube.glb#Cube").unwrap();
        assert_eq!(parsed.display(), "remote://models/cube.glb#Cube");
        assert_eq!(PyAssetPath::parse(&parsed.display()).unwrap(), parsed);
    }
}
