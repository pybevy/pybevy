use bevy::shader::ShaderImport;
use pybevy_core::repr::str_repr;
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
    fn __repr__(&self) -> String {
        match self {
            PyShaderImport::AssetPath { value } => {
                format!("ShaderImport.AssetPath({})", str_repr(value))
            }
            PyShaderImport::Custom { value } => format!("ShaderImport.Custom({})", str_repr(value)),
        }
    }
}
