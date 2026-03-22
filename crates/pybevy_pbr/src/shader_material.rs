//! Re-export shared ShaderMaterial types from pybevy_shader_types.
//!
//! The actual type definitions live in `pybevy_shader_types` so they can be
//! shared with `pybevy_replicon` and `pybevy_browser` without pulling in
//! Python dependencies.

pub use pybevy_shader_types::*;
