use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use bevy::{
    prelude::*,
    render::render_resource::{CachedPipelineState, PipelineCache},
    shader::{Shader, ShaderCacheError, ShaderImport},
};
use pybevy_core::public_error;

/// Compile errors of the render pipelines that currently fail, shared between
/// the render world, which publishes them, and the main world.
#[derive(Resource, Clone, Default)]
pub struct ShaderErrors(Arc<Mutex<Vec<String>>>);

impl ShaderErrors {
    pub fn current(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn replace(&self, errors: Vec<String>) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = errors;
    }
}

/// Publish the errors of pipelines Bevy gave up on. A pipeline is retried
/// when its shader changes, so a fixed shader clears its entry.
pub fn publish_shader_errors(cache: Res<PipelineCache>, errors: Res<ShaderErrors>) {
    let mut current: Vec<String> = cache
        .pipelines()
        .filter_map(|pipeline| match &pipeline.state {
            CachedPipelineState::Err(ShaderCacheError::ProcessShaderError(error)) => {
                Some(error.to_string())
            }
            CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(description)) => {
                Some(description.clone())
            }
            _ => None,
        })
        .collect();
    current.sort_unstable();
    current.dedup();
    errors.replace(current);
}

/// Custom `#import`s no loaded shader defines. Bevy keeps such pipelines queued
/// without an error, so the shader silently draws nothing.
#[derive(Resource, Default)]
pub struct UnresolvedShaderImports(Vec<String>);

pub fn track_unresolved_shader_imports(
    mut events: MessageReader<AssetEvent<Shader>>,
    shaders: Res<Assets<Shader>>,
    mut unresolved: ResMut<UnresolvedShaderImports>,
) {
    if events.read().count() == 0 {
        return;
    }
    let provided: HashSet<&str> = shaders
        .iter()
        .filter_map(|(_, shader)| match &shader.import_path {
            ShaderImport::Custom(path) => Some(path.as_str()),
            ShaderImport::AssetPath(_) => None,
        })
        .collect();
    // Engine shaders import modules of pipelines that may never be built.
    let mut missing: Vec<String> = shaders
        .iter()
        .filter(|(_, shader)| !shader.path.starts_with("embedded://"))
        .flat_map(|(_, shader)| {
            shader.imports.iter().filter_map(|import| match import {
                ShaderImport::Custom(path) if !provided.contains(path.as_str()) => {
                    Some(public_error::shader_import_unresolved(&shader.path, path))
                }
                _ => None,
            })
        })
        .collect();
    missing.sort_unstable();
    missing.dedup();
    unresolved.0 = missing;
}

/// Compile errors and unresolved imports, as `get_last_error.shader_errors` reports them.
pub fn current_shader_errors(world: &World) -> Vec<String> {
    let mut errors = world
        .get_resource::<ShaderErrors>()
        .map(ShaderErrors::current)
        .unwrap_or_default();
    if let Some(unresolved) = world.get_resource::<UnresolvedShaderImports>() {
        errors.extend(unresolved.0.iter().cloned());
    }
    errors
}
