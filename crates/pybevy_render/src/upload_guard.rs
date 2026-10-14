use bevy::{
    app::{App, Plugin},
    asset::AssetId,
    ecs::{
        schedule::IntoScheduleConfigs,
        system::{Res, ResMut},
    },
    image::Image,
    render::{
        Render, RenderApp, RenderSystems,
        render_asset::{ExtractedAssets, prepare_assets},
        renderer::RenderDevice,
        storage::{GpuShaderBuffer, ShaderBuffer},
        texture::GpuImage,
    },
};
use pybevy_core::public_error;
use tracing::error;
use wgpu_types::{Extent3d, Limits, TextureDimension};

/// Drops image and shader-buffer uploads the device must reject before Bevy
/// prepares them: wgpu panics on short image data and Bevy unwraps a failed
/// buffer mapping, and a rejected texture blanks every camera that samples it.
pub struct UploadGuardPlugin;

impl Plugin for UploadGuardPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                (
                    drop_unuploadable_images
                        .in_set(RenderSystems::PrepareAssets)
                        .before(prepare_assets::<GpuImage>),
                    drop_oversized_shader_buffers
                        .in_set(RenderSystems::PrepareAssets)
                        .before(prepare_assets::<GpuShaderBuffer>),
                ),
            );
        }
    }
}

fn drop_unuploadable_images(
    device: Option<Res<RenderDevice>>,
    extracted: Option<ResMut<ExtractedAssets<GpuImage>>>,
) {
    let (Some(device), Some(mut extracted)) = (device, extracted) else {
        return;
    };
    let limits = device.limits();
    extracted
        .extracted
        .retain(|(id, image)| match image_upload_problem(image, &limits) {
            Some(problem) => {
                error!(
                    "{}",
                    public_error::image_upload_rejected(&format!("{id:?}"), &problem)
                );
                false
            }
            None => true,
        });
}

fn drop_oversized_shader_buffers(
    device: Option<Res<RenderDevice>>,
    extracted: Option<ResMut<ExtractedAssets<GpuShaderBuffer>>>,
) {
    let (Some(device), Some(mut extracted)) = (device, extracted) else {
        return;
    };
    let limit = device.limits().max_buffer_size;
    extracted
        .extracted
        .retain(|(id, buffer): &(AssetId<ShaderBuffer>, ShaderBuffer)| {
            let size = buffer.buffer_size();
            if size > limit {
                error!(
                    "{}",
                    public_error::shader_buffer_upload_rejected(&format!("{id:?}"), size, limit)
                );
                return false;
            }
            true
        });
}

fn image_upload_problem(image: &Image, limits: &Limits) -> Option<String> {
    let descriptor = &image.texture_descriptor;
    let size = descriptor.size;
    if size.width == 0 || size.height == 0 || size.depth_or_array_layers == 0 {
        return Some(format!(
            "its size {}x{}x{} has a zero extent",
            size.width, size.height, size.depth_or_array_layers
        ));
    }
    if descriptor.usage.is_empty() {
        return Some("its texture usage is empty".to_string());
    }
    if descriptor.dimension == TextureDimension::D1
        && (size.height != 1 || size.depth_or_array_layers != 1)
    {
        return Some(format!(
            "it is 1D but {}x{}x{}, not one texel tall and one layer deep",
            size.width, size.height, size.depth_or_array_layers
        ));
    }
    if let Some(problem) = dimension_problem(descriptor.dimension, size, limits) {
        return Some(problem);
    }
    if descriptor.mip_level_count == 0
        || descriptor.mip_level_count > size.max_mips(descriptor.dimension)
    {
        return Some(format!(
            "mip_level_count {} is not possible for that size",
            descriptor.mip_level_count
        ));
    }
    let data = image.data.as_ref()?;
    match required_data_len(image) {
        Some(required) if data.len() >= required => None,
        Some(required) => Some(format!(
            "its data has {} bytes but the descriptor needs {required}",
            data.len()
        )),
        None => Some("its data size overflows".to_string()),
    }
}

fn dimension_problem(
    dimension: TextureDimension,
    size: Extent3d,
    limits: &Limits,
) -> Option<String> {
    let (axes, limit, layers) = match dimension {
        TextureDimension::D1 => ([size.width, 1, 1], limits.max_texture_dimension_1d, None),
        TextureDimension::D2 => (
            [size.width, size.height, 1],
            limits.max_texture_dimension_2d,
            Some(size.depth_or_array_layers),
        ),
        TextureDimension::D3 => (
            [size.width, size.height, size.depth_or_array_layers],
            limits.max_texture_dimension_3d,
            None,
        ),
    };
    if axes.iter().any(|&axis| axis > limit) {
        return Some(format!(
            "its size {}x{}x{} exceeds this GPU's {dimension:?} texture limit of {limit}",
            size.width, size.height, size.depth_or_array_layers
        ));
    }
    match layers {
        Some(layers) if layers > limits.max_texture_array_layers => Some(format!(
            "its {layers} array layers exceed this GPU's limit of {}",
            limits.max_texture_array_layers
        )),
        _ => None,
    }
}

/// Mirrors the byte walk of wgpu's `create_texture_with_data`, which slices
/// the data without a length check.
fn required_data_len(image: &Image) -> Option<usize> {
    let descriptor = &image.texture_descriptor;
    let format = descriptor.format;
    let block_size = u64::from(format.block_copy_size(None).unwrap_or(4));
    let (block_width, block_height) = format.block_dimensions();
    let mut total: u64 = 0;
    for mip in 0..descriptor.mip_level_count {
        let mut mip_size = descriptor.mip_level_size(mip)?;
        if descriptor.dimension != TextureDimension::D3 {
            mip_size.depth_or_array_layers = 1;
        }
        let physical = mip_size.physical_size(format);
        let blocks = u64::from(physical.width / block_width)
            .checked_mul(u64::from(physical.height / block_height))?;
        let mip_bytes = blocks
            .checked_mul(block_size)?
            .checked_mul(u64::from(mip_size.depth_or_array_layers))?;
        total = total.checked_add(mip_bytes)?;
    }
    let layers = u64::from(descriptor.array_layer_count());
    usize::try_from(total.checked_mul(layers)?).ok()
}
