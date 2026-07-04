use bevy::{asset::AssetPath, shader::ShaderRef};
use pybevy_core::PyHandle;
use pyo3::prelude::*;

#[pyclass(name = "ShaderRef", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub enum PyShaderRef {
    Default(),
    Handle(PyHandle),
    Path(String),
}

#[pymethods]
impl PyShaderRef {
    #[staticmethod]
    #[pyo3(name = "default")]
    pub fn default_() -> Self {
        PyShaderRef::Default()
    }

    #[staticmethod]
    pub fn from_handle(handle: PyHandle) -> Self {
        PyShaderRef::Handle(handle)
    }

    #[staticmethod]
    pub fn from_path(path: String) -> Self {
        PyShaderRef::Path(path)
    }

    fn __repr__(&self) -> String {
        match self {
            PyShaderRef::Default() => "ShaderRef.Default".to_string(),
            PyShaderRef::Handle(handle) => format!("ShaderRef.Handle({:?})", handle),
            PyShaderRef::Path(path) => format!("ShaderRef.Path(\"{}\")", path),
        }
    }

    fn __str__(&self) -> String {
        match self {
            PyShaderRef::Default() => "Default".to_string(),
            PyShaderRef::Handle(_) => "Handle".to_string(),
            PyShaderRef::Path(path) => format!("Path({})", path),
        }
    }
}

impl TryFrom<PyShaderRef> for ShaderRef {
    type Error = PyErr;

    fn try_from(py_ref: PyShaderRef) -> PyResult<Self> {
        match py_ref {
            PyShaderRef::Default() => Ok(ShaderRef::Default),
            PyShaderRef::Handle(handle) => Ok(ShaderRef::Handle(handle.try_into()?)),
            PyShaderRef::Path(path) => Ok(ShaderRef::Path(AssetPath::from(
                path.leak() as &'static str
            ))),
        }
    }
}

impl From<&ShaderRef> for PyShaderRef {
    fn from(shader_ref: &ShaderRef) -> Self {
        match shader_ref {
            ShaderRef::Default => PyShaderRef::Default(),
            ShaderRef::Handle(handle) => PyShaderRef::Handle(handle.into()),
            ShaderRef::Path(path) => PyShaderRef::Path(path.to_string()),
        }
    }
}
