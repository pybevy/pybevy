use bevy::shader::ShaderDefVal;
use pyo3::prelude::*;

#[pyclass(name = "ShaderDefVal", frozen, eq, from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyShaderDefVal {
    Bool { name: String, value: bool },
    Int { name: String, value: i32 },
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

impl From<PyShaderDefVal> for ShaderDefVal {
    fn from(py_def: PyShaderDefVal) -> Self {
        match py_def {
            PyShaderDefVal::Bool { name, value } => ShaderDefVal::Bool(name, value),
            PyShaderDefVal::Int { name, value } => ShaderDefVal::Int(name, value),
            PyShaderDefVal::UInt { name, value } => ShaderDefVal::UInt(name, value),
        }
    }
}

impl From<&ShaderDefVal> for PyShaderDefVal {
    fn from(def: &ShaderDefVal) -> Self {
        match def {
            ShaderDefVal::Bool(name, value) => PyShaderDefVal::Bool {
                name: name.clone(),
                value: *value,
            },
            ShaderDefVal::Int(name, value) => PyShaderDefVal::Int {
                name: name.clone(),
                value: *value,
            },
            ShaderDefVal::UInt(name, value) => PyShaderDefVal::UInt {
                name: name.clone(),
                value: *value,
            },
        }
    }
}
