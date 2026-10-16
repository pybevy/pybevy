use std::sync::{Arc, Mutex};

use bevy::{
    prelude::*,
    render::render_resource::{CachedPipelineState, PipelineCache},
    shader::ShaderCacheError,
};

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
