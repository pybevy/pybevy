use bevy::shader::ShaderDefVal;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ShaderDefVal, no_repr)]
#[pyclass(
    name = "ShaderDefVal",
    module = "pybevy.shader",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyShaderDefVal {
    #[py_bevy(tuple)]
    Bool { name: String, value: bool },
    #[py_bevy(tuple)]
    Int { name: String, value: i32 },
    #[py_bevy(tuple)]
    UInt { name: String, value: u32 },
}

#[pymethods]
impl PyShaderDefVal {
    pub fn value_as_string(&self) -> String {
        match self {
            PyShaderDefVal::Bool { value, .. } => value.to_string(),
            PyShaderDefVal::Int { value, .. } => value.to_string(),
            PyShaderDefVal::UInt { value, .. } => value.to_string(),
        }
    }

    fn __repr__(&self) -> String {
        match self {
            PyShaderDefVal::Bool { name, value } => {
                format!("ShaderDefVal.Bool(\"{}\", {})", name, value)
            }
            PyShaderDefVal::Int { name, value } => {
                format!("ShaderDefVal.Int(\"{}\", {})", name, value)
            }
            PyShaderDefVal::UInt { name, value } => {
                format!("ShaderDefVal.UInt(\"{}\", {})", name, value)
            }
        }
    }

    fn __str__(&self) -> String {
        let name = match self {
            PyShaderDefVal::Bool { name, .. }
            | PyShaderDefVal::Int { name, .. }
            | PyShaderDefVal::UInt { name, .. } => name,
        };
        format!("{}={}", name, self.value_as_string())
    }
}
