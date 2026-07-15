use std::borrow::Cow;

use bevy::shader::Source;
use naga::ShaderStage;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

#[pyclass(name = "Source", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub enum PySource {
    Wgsl { value: String },
    Wesl { value: String },
    GlslVertex { value: String },
    GlslFragment { value: String },
    GlslCompute { value: String },
    SpirV { value: Vec<u8> },
}

#[pymethods]
impl PySource {
    pub fn as_str(&self) -> PyResult<String> {
        match self {
            PySource::Wgsl { value }
            | PySource::Wesl { value }
            | PySource::GlslVertex { value }
            | PySource::GlslFragment { value }
            | PySource::GlslCompute { value } => Ok(value.clone()),
            PySource::SpirV { .. } => Err(PyRuntimeError::new_err(
                "Cannot get string from SPIR-V bytecode",
            )),
        }
    }

    fn __repr__(&self) -> String {
        match self {
            PySource::Wgsl { value } => format!("Source.Wgsl({} chars)", value.len()),
            PySource::Wesl { value } => format!("Source.Wesl({} chars)", value.len()),
            PySource::GlslVertex { value } => {
                format!("Source.GlslVertex({} chars)", value.len())
            }
            PySource::GlslFragment { value } => {
                format!("Source.GlslFragment({} chars)", value.len())
            }
            PySource::GlslCompute { value } => {
                format!("Source.GlslCompute({} chars)", value.len())
            }
            PySource::SpirV { value } => format!("Source.SpirV({} bytes)", value.len()),
        }
    }
}

impl From<PySource> for Source {
    fn from(py_source: PySource) -> Self {
        match py_source {
            PySource::Wgsl { value } => Source::Wgsl(Cow::Owned(value)),
            PySource::Wesl { value } => Source::Wesl(Cow::Owned(value)),
            PySource::GlslVertex { value } => Source::Glsl(Cow::Owned(value), ShaderStage::Vertex),
            PySource::GlslFragment { value } => {
                Source::Glsl(Cow::Owned(value), ShaderStage::Fragment)
            }
            PySource::GlslCompute { value } => {
                Source::Glsl(Cow::Owned(value), ShaderStage::Compute)
            }
            PySource::SpirV { value } => Source::SpirV(Cow::Owned(value)),
        }
    }
}

impl From<&Source> for PySource {
    fn from(source: &Source) -> Self {
        match source {
            Source::Wgsl(s) => PySource::Wgsl {
                value: s.to_string(),
            },
            Source::Wesl(s) => PySource::Wesl {
                value: s.to_string(),
            },
            Source::Glsl(s, stage) => match stage {
                ShaderStage::Vertex => PySource::GlslVertex {
                    value: s.to_string(),
                },
                ShaderStage::Fragment => PySource::GlslFragment {
                    value: s.to_string(),
                },
                ShaderStage::Compute => PySource::GlslCompute {
                    value: s.to_string(),
                },
                _ => PySource::Wgsl {
                    value: format!("// Unsupported GLSL stage: {:?}", stage),
                },
            },
            Source::SpirV(b) => PySource::SpirV { value: b.to_vec() },
        }
    }
}
