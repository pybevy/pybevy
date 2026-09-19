use bevy::{
    app::{App, Plugin},
    ecs::{
        schedule::IntoScheduleConfigs,
        system::{Query, Res, ResMut},
    },
    pbr::{BinUnpackingBindGroups, ViewKeyCache, ViewKeyPrepassCache},
    platform::collections::{HashMap, HashSet},
    render::{
        Render, RenderApp, RenderSystems,
        batching::gpu_preprocessing::{BinUnpackingBuffers, BinUnpackingBuffersKey},
        view::ExtractedView,
    },
};

/// Releases caches for retired render views.
pub struct RenderCacheCleanupPlugin;

impl Plugin for RenderCacheCleanupPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                (prune_bin_bind_groups, prune_view_keys).after(RenderSystems::Cleanup),
            );
        }
    }
}

fn prune_view_keys(
    views: Query<&ExtractedView>,
    keys: Option<ResMut<ViewKeyCache>>,
    prepass_keys: Option<ResMut<ViewKeyPrepassCache>>,
) {
    if keys.is_none() && prepass_keys.is_none() {
        return;
    }
    let live: HashSet<_> = views.iter().map(|view| view.retained_view_entity).collect();
    if let Some(mut keys) = keys {
        keys.retain(|view, _| live.contains(view));
    }
    if let Some(mut keys) = prepass_keys {
        keys.retain(|view, _| live.contains(view));
    }
}

fn prune_bin_bind_groups(
    buffers: Option<Res<BinUnpackingBuffers>>,
    groups: Option<ResMut<BinUnpackingBindGroups>>,
) {
    if let (Some(buffers), Some(mut groups)) = (buffers, groups) {
        // Bevy 0.19.1 prunes the buffers but leaves their bind groups alive.
        retain_buffer_keys(&buffers, &mut groups);
    }
}

fn retain_buffer_keys<T>(
    buffers: &BinUnpackingBuffers,
    groups: &mut HashMap<BinUnpackingBuffersKey, T>,
) {
    groups.retain(|key, _| buffers.view_phase_buffers.contains_key(key));
}
