use bevy::shader::ShaderDefVal;
use pyo3::prelude::*;

#[pyclass(name = "ShaderDefVal", frozen, eq, from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PyShaderDefVal {
    Bool(String, bool),
    Int(String, i32),
    UInt(String, u32),
}

#[pymethods]
impl PyShaderDefVal {
    #[staticmethod]
    #[pyo3(name = "bool")]
    pub fn bool_(name: String, value: bool) -> Self {
        PyShaderDefVal::Bool(name, value)
    }

    #[staticmethod]
    #[pyo3(name = "int")]
    pub fn int_(name: String, value: i32) -> Self {
        PyShaderDefVal::Int(name, value)
    }

    #[staticmethod]
    #[pyo3(name = "uint")]
    pub fn uint_(name: String, value: u32) -> Self {
        PyShaderDefVal::UInt(name, value)
    }

    #[getter]
    pub fn name(&self) -> String {
        match self {
            PyShaderDefVal::Bool(name, _) => name.clone(),
            PyShaderDefVal::Int(name, _) => name.clone(),
            PyShaderDefVal::UInt(name, _) => name.clone(),
        }
    }

    pub fn value_as_string(&self) -> String {
        match self {
            PyShaderDefVal::Bool(_, v) => v.to_string(),
            PyShaderDefVal::Int(_, v) => v.to_string(),
            PyShaderDefVal::UInt(_, v) => v.to_string(),
        }
    }

    fn __repr__(&self) -> String {
        match self {
            PyShaderDefVal::Bool(name, value) => {
                format!("ShaderDefVal.bool(\"{}\", {})", name, value)
            }
            PyShaderDefVal::Int(name, value) => {
                format!("ShaderDefVal.int(\"{}\", {})", name, value)
            }
            PyShaderDefVal::UInt(name, value) => {
                format!("ShaderDefVal.uint(\"{}\", {})", name, value)
            }
        }
    }

    fn __str__(&self) -> String {
        format!("{}={}", self.name(), self.value_as_string())
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

impl From<PyShaderDefVal> for ShaderDefVal {
    fn from(py_def: PyShaderDefVal) -> Self {
        match py_def {
            PyShaderDefVal::Bool(name, value) => ShaderDefVal::Bool(name, value),
            PyShaderDefVal::Int(name, value) => ShaderDefVal::Int(name, value),
            PyShaderDefVal::UInt(name, value) => ShaderDefVal::UInt(name, value),
        }
    }
}

impl From<&ShaderDefVal> for PyShaderDefVal {
    fn from(def: &ShaderDefVal) -> Self {
        match def {
            ShaderDefVal::Bool(name, value) => PyShaderDefVal::Bool(name.clone(), *value),
            ShaderDefVal::Int(name, value) => PyShaderDefVal::Int(name.clone(), *value),
            ShaderDefVal::UInt(name, value) => PyShaderDefVal::UInt(name.clone(), *value),
        }
    }
}
