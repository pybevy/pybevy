use bevy::render::render_resource::TextureUsages;
use pybevy_core::public_error::{
    camera_depth_texture_usage_invalid, camera_main_texture_usages_invalid,
};

/// wgpu rejects the camera's main texture without these usages, blanking the camera.
pub fn check_main_texture_usages(usages: TextureUsages) -> Result<TextureUsages, String> {
    let required = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
    if usages.contains(required) && !usages.contains(TextureUsages::TRANSIENT_ATTACHMENT) {
        Ok(usages)
    } else {
        Err(camera_main_texture_usages_invalid(&format!("{usages:?}")))
    }
}

/// wgpu rejects a depth texture that is not a render attachment or asks for
/// storage or transient usage.
pub fn check_depth_texture_usage(value: u32) -> Result<TextureUsages, String> {
    let forbidden = TextureUsages::STORAGE_BINDING
        | TextureUsages::STORAGE_ATOMIC
        | TextureUsages::TRANSIENT_ATTACHMENT;
    match TextureUsages::from_bits(value) {
        Some(usages)
            if usages.contains(TextureUsages::RENDER_ATTACHMENT)
                && !usages.intersects(forbidden) =>
        {
            Ok(usages)
        }
        _ => Err(camera_depth_texture_usage_invalid(value)),
    }
}
