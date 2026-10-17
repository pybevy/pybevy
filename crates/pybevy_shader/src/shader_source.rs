use std::borrow::Cow;

use bevy::shader::Source;
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

#[pyenum(Source, manual)]
#[pyclass(
    name = "Source",
    module = "pybevy.shader",
    frozen,
    skip_from_py_object,
    eq
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PySource {
    Wgsl { value: String },
    Wesl { value: String },
    SpirV { value: Vec<u8> },
}

#[pymethods]
impl PySource {
    pub fn as_str(&self) -> PyResult<String> {
        match self {
            PySource::Wgsl { value } | PySource::Wesl { value } => Ok(value.clone()),
            PySource::SpirV { .. } => Err(PyRuntimeError::new_err(
                "Cannot get string from SPIR-V bytecode",
            )),
        }
    }

    fn __repr__(&self) -> String {
        match self {
            PySource::Wgsl { value } => format!("Source.Wgsl({} chars)", value.len()),
            PySource::Wesl { value } => format!("Source.Wesl({} chars)", value.len()),
            PySource::SpirV { value } => format!("Source.SpirV({} bytes)", value.len()),
        }
    }
}

impl From<PySource> for Source {
    fn from(py_source: PySource) -> Self {
        match py_source {
            PySource::Wgsl { value } => Source::Wgsl(Cow::Owned(value)),
            PySource::Wesl { value } => Source::Wesl(Cow::Owned(value)),
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
            Source::SpirV(b) => PySource::SpirV { value: b.to_vec() },
        }
    }
}
