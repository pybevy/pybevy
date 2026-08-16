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
    GlslTask { value: String },
    GlslRayGeneration { value: String },
    GlslMiss { value: String },
    GlslAnyHit { value: String },
    GlslClosestHit { value: String },
    GlslMesh { value: String },
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
            | PySource::GlslTask { value }
            | PySource::GlslRayGeneration { value }
            | PySource::GlslMiss { value }
            | PySource::GlslAnyHit { value }
            | PySource::GlslClosestHit { value }
            | PySource::GlslMesh { value }
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
            PySource::GlslTask { value } => {
                format!("Source.GlslTask({} chars)", value.len())
            }
            PySource::GlslRayGeneration { value } => {
                format!("Source.GlslRayGeneration({} chars)", value.len())
            }
            PySource::GlslMiss { value } => {
                format!("Source.GlslMiss({} chars)", value.len())
            }
            PySource::GlslAnyHit { value } => {
                format!("Source.GlslAnyHit({} chars)", value.len())
            }
            PySource::GlslClosestHit { value } => {
                format!("Source.GlslClosestHit({} chars)", value.len())
            }
            PySource::GlslMesh { value } => {
                format!("Source.GlslMesh({} chars)", value.len())
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
            PySource::GlslTask { value } => Source::Glsl(Cow::Owned(value), ShaderStage::Task),
            PySource::GlslRayGeneration { value } => {
                Source::Glsl(Cow::Owned(value), ShaderStage::RayGeneration)
            }
            PySource::GlslMiss { value } => Source::Glsl(Cow::Owned(value), ShaderStage::Miss),
            PySource::GlslAnyHit { value } => Source::Glsl(Cow::Owned(value), ShaderStage::AnyHit),
            PySource::GlslClosestHit { value } => {
                Source::Glsl(Cow::Owned(value), ShaderStage::ClosestHit)
            }
            PySource::GlslMesh { value } => Source::Glsl(Cow::Owned(value), ShaderStage::Mesh),
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
                ShaderStage::Task => PySource::GlslTask {
                    value: s.to_string(),
                },
                ShaderStage::RayGeneration => PySource::GlslRayGeneration {
                    value: s.to_string(),
                },
                ShaderStage::Miss => PySource::GlslMiss {
                    value: s.to_string(),
                },
                ShaderStage::AnyHit => PySource::GlslAnyHit {
                    value: s.to_string(),
                },
                ShaderStage::ClosestHit => PySource::GlslClosestHit {
                    value: s.to_string(),
                },
                ShaderStage::Mesh => PySource::GlslMesh {
                    value: s.to_string(),
                },
                ShaderStage::Fragment => PySource::GlslFragment {
                    value: s.to_string(),
                },
                ShaderStage::Compute => PySource::GlslCompute {
                    value: s.to_string(),
                },
            },
            Source::SpirV(b) => PySource::SpirV { value: b.to_vec() },
        }
    }
}
