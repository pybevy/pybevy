use bevy::material::AlphaMode;
use pyo3::prelude::*;

#[pyclass(name = "AlphaMode", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyAlphaMode {
    Opaque(),
    Mask { value: f32 },
    Blend(),
    Premultiplied(),
    Add(),
    Multiply(),
    AlphaToCoverage(),
}

impl From<PyAlphaMode> for AlphaMode {
    fn from(mode: PyAlphaMode) -> Self {
        match mode {
            PyAlphaMode::Opaque() => AlphaMode::Opaque,
            PyAlphaMode::Mask { value } => AlphaMode::Mask(value),
            PyAlphaMode::Blend() => AlphaMode::Blend,
            PyAlphaMode::Premultiplied() => AlphaMode::Premultiplied,
            PyAlphaMode::Add() => AlphaMode::Add,
            PyAlphaMode::Multiply() => AlphaMode::Multiply,
            PyAlphaMode::AlphaToCoverage() => AlphaMode::AlphaToCoverage,
        }
    }
}

impl From<AlphaMode> for PyAlphaMode {
    fn from(mode: AlphaMode) -> Self {
        match mode {
            AlphaMode::Opaque => PyAlphaMode::Opaque(),
            AlphaMode::Mask(value) => PyAlphaMode::Mask { value },
            AlphaMode::Blend => PyAlphaMode::Blend(),
            AlphaMode::Premultiplied => PyAlphaMode::Premultiplied(),
            AlphaMode::Add => PyAlphaMode::Add(),
            AlphaMode::Multiply => PyAlphaMode::Multiply(),
            AlphaMode::AlphaToCoverage => PyAlphaMode::AlphaToCoverage(),
        }
    }
}
