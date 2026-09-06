use bevy::{asset::AssetPath, shader::ShaderRef};
use pybevy_core::PyHandle;
use pyo3::prelude::*;

#[pyclass(
    name = "ShaderRef",
    module = "pybevy.shader",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub enum PyShaderRef {
    Default(),
    Handle { value: PyHandle },
    Path { value: String },
}

#[pymethods]
impl PyShaderRef {
    #[staticmethod]
    #[pyo3(name = "default")]
    pub fn default_() -> Self {
        Self::Default()
    }

    fn __repr__(&self) -> String {
        match self {
            PyShaderRef::Default() => "ShaderRef.Default".to_string(),
            PyShaderRef::Handle { value } => format!("ShaderRef.Handle({:?})", value),
            PyShaderRef::Path { value } => format!("ShaderRef.Path(\"{}\")", value),
        }
    }

    fn __str__(&self) -> String {
        match self {
            PyShaderRef::Default() => "Default".to_string(),
            PyShaderRef::Handle { .. } => "Handle".to_string(),
            PyShaderRef::Path { value } => format!("Path({})", value),
        }
    }
}

impl TryFrom<PyShaderRef> for ShaderRef {
    type Error = PyErr;

    fn try_from(py_ref: PyShaderRef) -> PyResult<Self> {
        match py_ref {
            PyShaderRef::Default() => Ok(ShaderRef::Default),
            PyShaderRef::Handle { value } => Ok(ShaderRef::Handle(value.try_into()?)),
            PyShaderRef::Path { value } => Ok(ShaderRef::Path(AssetPath::from(value))),
        }
    }
}

impl From<&ShaderRef> for PyShaderRef {
    fn from(shader_ref: &ShaderRef) -> Self {
        match shader_ref {
            ShaderRef::Default => PyShaderRef::Default(),
            ShaderRef::Handle(handle) => PyShaderRef::Handle {
                value: handle.into(),
            },
            ShaderRef::Path(path) => PyShaderRef::Path {
                value: path.to_string(),
            },
        }
    }
}
