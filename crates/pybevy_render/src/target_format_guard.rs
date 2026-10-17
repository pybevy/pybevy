use bevy::{
    app::{App, Plugin, PostUpdate},
    asset::Assets,
    camera::{Camera, RenderTarget},
    ecs::system::{Query, Res},
    image::Image,
    log::error,
    render::render_resource::{TextureFormat, TextureSampleType},
};

/// Whether Bevy's camera pipelines can render into `format`: float-sampled
/// colour formats. Integer, depth, stencil and shared-exponent targets make
/// Bevy panic while specializing the mesh pipelines.
pub fn is_camera_renderable(format: TextureFormat) -> bool {
    !format.has_depth_aspect()
        && !format.has_stencil_aspect()
        && format != TextureFormat::Rgb9e5Ufloat
        && matches!(
            format.sample_type(None, None),
            Some(TextureSampleType::Float { .. })
        )
}

pub fn unrenderable_target_message(format: TextureFormat) -> String {
    format!(
        "TextureFormat.{format:?} cannot be a camera render target; use a float, unorm or \
         snorm colour format such as TextureFormat.Rgba8UnormSrgb"
    )
}

/// Deactivates cameras whose image target Bevy cannot render into, which
/// would otherwise panic the render world.
pub struct TargetFormatGuardPlugin;

impl Plugin for TargetFormatGuardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, deactivate_unrenderable_targets);
    }
}

fn deactivate_unrenderable_targets(
    images: Option<Res<Assets<Image>>>,
    mut cameras: Query<(&mut Camera, &RenderTarget)>,
) {
    let Some(images) = images else {
        return;
    };
    for (mut camera, target) in &mut cameras {
        let RenderTarget::Image(target) = target else {
            continue;
        };
        if !camera.is_active {
            continue;
        }
        let Some(image) = images.get(&target.handle) else {
            continue;
        };
        let format = image.texture_descriptor.format;
        if !is_camera_renderable(format) {
            error!(
                "{}; the camera was deactivated",
                unrenderable_target_message(format)
            );
            camera.is_active = false;
        }
    }
}
