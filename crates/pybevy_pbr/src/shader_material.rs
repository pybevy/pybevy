//! Re-export shared ShaderMaterial types from pybevy_shader_types.
//!
//! The actual type definitions live in `pybevy_shader_types`, which has no
//! Python dependencies, so native-only crates can use them.

pub use pybevy_shader_types::*;
