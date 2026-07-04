use bevy::material::AlphaMode;
use pyo3::prelude::*;

#[pyclass(name = "AlphaMode", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyAlphaMode {
    Opaque(),
    Mask(f32),
    Blend(),
    Premultiplied(),
    Add(),
    Multiply(),
    AlphaToCoverage(),
}

#[pymethods]
impl PyAlphaMode {
    #[classattr]
    pub const OPAQUE: Self = PyAlphaMode::Opaque();

    #[classattr]
    pub const BLEND: Self = PyAlphaMode::Blend();

    #[classattr]
    pub const PREMULTIPLIED: Self = PyAlphaMode::Premultiplied();

    #[classattr]
    pub const ADD: Self = PyAlphaMode::Add();

    #[classattr]
    pub const MULTIPLY: Self = PyAlphaMode::Multiply();

    #[classattr]
    pub const ALPHA_TO_COVERAGE: Self = PyAlphaMode::AlphaToCoverage();
}

impl From<PyAlphaMode> for AlphaMode {
    fn from(mode: PyAlphaMode) -> Self {
        match mode {
            PyAlphaMode::Opaque() => AlphaMode::Opaque,
            PyAlphaMode::Mask(mask) => AlphaMode::Mask(mask),
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
            AlphaMode::Mask(f32) => PyAlphaMode::Mask(f32),
            AlphaMode::Blend => PyAlphaMode::Blend(),
            AlphaMode::Premultiplied => PyAlphaMode::Premultiplied(),
            AlphaMode::Add => PyAlphaMode::Add(),
            AlphaMode::Multiply => PyAlphaMode::Multiply(),
            AlphaMode::AlphaToCoverage => PyAlphaMode::AlphaToCoverage(),
        }
    }
}
