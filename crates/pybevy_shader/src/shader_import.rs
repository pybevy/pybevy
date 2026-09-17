use bevy::shader::ShaderImport;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ShaderImport, no_repr)]
#[pyclass(
    name = "ShaderImport",
    module = "pybevy.shader",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyShaderImport {
    #[py_bevy(tuple)]
    AssetPath { value: String },
    #[py_bevy(tuple)]
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
}
