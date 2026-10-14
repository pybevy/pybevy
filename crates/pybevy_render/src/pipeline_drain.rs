//! Finish in-flight pipeline compilation before an App releases its renderer,
//! so no async compute worker drops the last render device reference later.

use bevy::{
    app::{App, AppExit, Plugin},
    ecs::{
        message::Messages,
        resource::Resource,
        schedule::IntoScheduleConfigs,
        system::{Res, ResMut},
        world::World,
    },
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        pipelined_rendering::RenderAppChannels,
        render_resource::{CachedPipelineState, CachedRenderPipelineId, PipelineCache},
    },
    tasks::block_on,
};

/// Waits for pipeline compilation in the render frame that follows an exit request.
pub struct PipelineCompilationDrainPlugin;

impl Plugin for PipelineCompilationDrainPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<ExitRequested>()
                .add_systems(ExtractSchedule, extract_exit_requested)
                .add_systems(Render, drain_on_exit_request.in_set(RenderSystems::Cleanup));
        }
    }
}

/// Whether the main world held an `AppExit` message when this frame was extracted.
#[derive(Resource, Default)]
struct ExitRequested(bool);

fn extract_exit_requested(
    mut requested: ResMut<ExitRequested>,
    exits: Extract<Option<Res<Messages<AppExit>>>>,
) {
    // Same condition as `App::should_exit`, which ends the runner after this frame.
    requested.0 = exits
        .as_deref()
        .is_some_and(|messages| !messages.is_empty());
}

fn drain_on_exit_request(requested: Res<ExitRequested>, cache: Option<ResMut<PipelineCache>>) {
    if requested.0
        && let Some(mut cache) = cache
    {
        drain_pipeline_cache(&mut cache);
    }
}

/// Block until every started compilation task has finished; queued pipelines stay unstarted.
fn drain_pipeline_cache(cache: &mut PipelineCache) {
    let creating: Vec<usize> = cache
        .pipelines()
        .enumerate()
        .filter(|(_, pipeline)| matches!(pipeline.state, CachedPipelineState::Creating(_)))
        .map(|(index, _)| index)
        .collect();
    for index in creating {
        // Render and compute pipelines share one index space; this waits on either kind.
        cache.block_on_render_pipeline(CachedRenderPipelineId::new(index));
    }
}

/// Finish the App's in-flight pipeline compilation before it is dropped.
///
/// A pipelined render sub-app is received from the render thread and dropped
/// here, as `RenderAppChannels` would do when the App is dropped.
pub fn drain_app_pipeline_compilation(app: &mut App) {
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        drain_world(render_app.world_mut());
        return;
    }
    let Some(mut channels) = app.world_mut().get_resource_mut::<RenderAppChannels>() else {
        return;
    };
    if let Some(mut render_app) = block_on(channels.recv()) {
        drain_world(render_app.world_mut());
    }
}

fn drain_world(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<PipelineCache>() {
        drain_pipeline_cache(&mut cache);
    }
}
