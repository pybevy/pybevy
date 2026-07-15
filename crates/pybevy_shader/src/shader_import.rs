use bevy::shader::ShaderImport;
use pyo3::prelude::*;

#[pyclass(name = "ShaderImport", frozen, eq, from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PyShaderImport {
    AssetPath { value: String },
    Custom { value: String },
}

#[pymethods]
impl PyShaderImport {
    pub fn module_name(&self) -> String {
        match self {
            PyShaderImport::AssetPath { value } => format!("\"{value}\""),
            PyShaderImport::Custom { value } => value.clone(),
        }
    }

    fn __repr__(&self) -> String {
        match self {
            PyShaderImport::AssetPath { value } => {
                format!("ShaderImport.AssetPath(\"{}\")", value)
            }
            PyShaderImport::Custom { value } => format!("ShaderImport.Custom(\"{}\")", value),
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
            PyShaderImport::AssetPath { value } => ShaderImport::AssetPath(value),
            PyShaderImport::Custom { value } => ShaderImport::Custom(value),
        }
    }
}

impl From<&ShaderImport> for PyShaderImport {
    fn from(import: &ShaderImport) -> Self {
        match import {
            ShaderImport::AssetPath(value) => PyShaderImport::AssetPath {
                value: value.clone(),
            },
            ShaderImport::Custom(value) => PyShaderImport::Custom {
                value: value.clone(),
            },
        }
    }
}

impl From<ShaderImport> for PyShaderImport {
    fn from(import: ShaderImport) -> Self {
        match import {
            ShaderImport::AssetPath(value) => PyShaderImport::AssetPath { value },
            ShaderImport::Custom(value) => PyShaderImport::Custom { value },
        }
    }
}
