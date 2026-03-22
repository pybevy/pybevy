use std::borrow::Cow;

use bevy::shader::Source;
use naga::ShaderStage;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

#[pyclass(name = "Source", frozen)]
#[derive(Debug, Clone)]
pub enum PySource {
    Wgsl(String),
    Wesl(String),
    GlslVertex(String),
    GlslFragment(String),
    GlslCompute(String),
    SpirV(Vec<u8>),
}

#[pymethods]
impl PySource {
    #[staticmethod]
    pub fn wgsl(source: String) -> Self {
        PySource::Wgsl(source)
    }

    #[staticmethod]
    pub fn wesl(source: String) -> Self {
        PySource::Wesl(source)
    }

    #[staticmethod]
    pub fn glsl_vertex(source: String) -> Self {
        PySource::GlslVertex(source)
    }

    #[staticmethod]
    pub fn glsl_fragment(source: String) -> Self {
        PySource::GlslFragment(source)
    }

    #[staticmethod]
    pub fn glsl_compute(source: String) -> Self {
        PySource::GlslCompute(source)
    }

    #[staticmethod]
    pub fn spirv(bytecode: Vec<u8>) -> Self {
        PySource::SpirV(bytecode)
    }

    pub fn as_str(&self) -> PyResult<String> {
        match self {
            PySource::Wgsl(s)
            | PySource::Wesl(s)
            | PySource::GlslVertex(s)
            | PySource::GlslFragment(s)
            | PySource::GlslCompute(s) => Ok(s.clone()),
            PySource::SpirV(_) => Err(PyRuntimeError::new_err(
                "Cannot get string from SPIR-V bytecode",
            )),
        }
    }

    fn __repr__(&self) -> String {
        match self {
            PySource::Wgsl(s) => format!("Source.Wgsl({} chars)", s.len()),
            PySource::Wesl(s) => format!("Source.Wesl({} chars)", s.len()),
            PySource::GlslVertex(s) => format!("Source.GlslVertex({} chars)", s.len()),
            PySource::GlslFragment(s) => format!("Source.GlslFragment({} chars)", s.len()),
            PySource::GlslCompute(s) => format!("Source.GlslCompute({} chars)", s.len()),
            PySource::SpirV(b) => format!("Source.SpirV({} bytes)", b.len()),
        }
    }
}

impl From<PySource> for Source {
    fn from(py_source: PySource) -> Self {
        match py_source {
            PySource::Wgsl(s) => Source::Wgsl(Cow::Owned(s)),
            PySource::Wesl(s) => Source::Wesl(Cow::Owned(s)),
            PySource::GlslVertex(s) => Source::Glsl(Cow::Owned(s), ShaderStage::Vertex),
            PySource::GlslFragment(s) => Source::Glsl(Cow::Owned(s), ShaderStage::Fragment),
            PySource::GlslCompute(s) => Source::Glsl(Cow::Owned(s), ShaderStage::Compute),
            PySource::SpirV(b) => Source::SpirV(Cow::Owned(b)),
        }
    }
}

impl From<&Source> for PySource {
    fn from(source: &Source) -> Self {
        match source {
            Source::Wgsl(s) => PySource::Wgsl(s.to_string()),
            Source::Wesl(s) => PySource::Wesl(s.to_string()),
            Source::Glsl(s, stage) => match stage {
                ShaderStage::Vertex => PySource::GlslVertex(s.to_string()),
                ShaderStage::Fragment => PySource::GlslFragment(s.to_string()),
                ShaderStage::Compute => PySource::GlslCompute(s.to_string()),
                _ => PySource::Wgsl(format!("// Unsupported GLSL stage: {:?}", stage)),
            },
            Source::SpirV(b) => PySource::SpirV(b.to_vec()),
        }
    }
}
