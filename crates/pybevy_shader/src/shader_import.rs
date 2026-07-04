use bevy::shader::ShaderImport;
use pyo3::prelude::*;

#[pyclass(name = "ShaderImport", frozen, eq, from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PyShaderImport {
    AssetPath(String),
    Custom(String),
}

#[pymethods]
impl PyShaderImport {
    #[staticmethod]
    pub fn asset_path(path: String) -> Self {
        PyShaderImport::AssetPath(path)
    }

    #[staticmethod]
    pub fn custom(name: String) -> Self {
        PyShaderImport::Custom(name)
    }

    pub fn module_name(&self) -> String {
        match self {
            PyShaderImport::AssetPath(s) => format!("\"{s}\""),
            PyShaderImport::Custom(s) => s.clone(),
        }
    }

    fn __repr__(&self) -> String {
        match self {
            PyShaderImport::AssetPath(path) => format!("ShaderImport.AssetPath(\"{}\")", path),
            PyShaderImport::Custom(name) => format!("ShaderImport.Custom(\"{}\")", name),
        }
    }

    fn __str__(&self) -> String {
        self.module_name()
    }

    fn __hash__(&self) -> u64 {
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<PyShaderImport> for ShaderImport {
    fn from(py_import: PyShaderImport) -> Self {
        match py_import {
            PyShaderImport::AssetPath(path) => ShaderImport::AssetPath(path),
            PyShaderImport::Custom(name) => ShaderImport::Custom(name),
        }
    }
}

impl From<&ShaderImport> for PyShaderImport {
    fn from(import: &ShaderImport) -> Self {
        match import {
            ShaderImport::AssetPath(path) => PyShaderImport::AssetPath(path.clone()),
            ShaderImport::Custom(name) => PyShaderImport::Custom(name.clone()),
        }
    }
}

impl From<ShaderImport> for PyShaderImport {
    fn from(import: ShaderImport) -> Self {
        match import {
            ShaderImport::AssetPath(path) => PyShaderImport::AssetPath(path),
            ShaderImport::Custom(name) => PyShaderImport::Custom(name),
        }
    }
}
