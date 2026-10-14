use bevy::{
    app::{App, Plugin},
    ecs::{
        schedule::IntoScheduleConfigs,
        system::{Res, ResMut},
    },
    light::{DirectionalLightShadowMap, PointLightShadowMap},
    render::{Render, RenderApp, RenderSystems, renderer::RenderDevice},
};
use tracing::warn;

/// Caps the render world's shadow map sizes at the device's 2D texture limit,
/// which Bevy 0.20 applies only to the directional texture, not its viewports.
pub struct ShadowMapLimitPlugin;

impl Plugin for ShadowMapLimitPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                clamp_shadow_map_sizes.in_set(RenderSystems::PrepareAssets),
            );
        }
    }
}

fn clamp_shadow_map_sizes(
    device: Option<Res<RenderDevice>>,
    point: Option<ResMut<PointLightShadowMap>>,
    directional: Option<ResMut<DirectionalLightShadowMap>>,
) {
    let Some(device) = device else {
        return;
    };
    let limit = device.limits().max_texture_dimension_2d as usize;
    if let Some(mut map) = point {
        clamp("PointLightShadowMap", &mut map.size, limit);
    }
    if let Some(mut map) = directional {
        clamp("DirectionalLightShadowMap", &mut map.size, limit);
    }
}

fn clamp(resource: &str, size: &mut usize, limit: usize) {
    if *size > limit {
        warn!(
            "{resource}.size {size} exceeds this GPU's 2D texture limit {limit}; rendering with {limit}"
        );
        *size = limit;
    }
}
