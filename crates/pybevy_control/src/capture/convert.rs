use bevy::{image::Image, render::render_resource::TextureFormat};
use half::f16;
use image::{DynamicImage, ImageBuffer, Rgba};

/// Convert a captured target to an image, decoding the linear and signed
/// colour formats Bevy's own conversion rejects into 32-bit float RGBA.
/// Single- and two-channel formats become grey from their first channel.
pub fn capture_to_dynamic(image: Image) -> Result<DynamicImage, String> {
    let format = image.texture_descriptor.format;
    let (width, height) = (image.width(), image.height());
    let Some(data) = image.data.as_deref() else {
        return Err(format!("captured {format:?} image has no data"));
    };
    let Some(decode) = decoder(format) else {
        return image
            .try_into_dynamic()
            .map_err(|error| format!("{error:?}"));
    };
    let stride = format
        .block_copy_size(None)
        .ok_or_else(|| format!("captured {format:?} image has no pixel size"))?
        as usize;
    let mut pixels = Vec::with_capacity(data.len() / stride * 4);
    for chunk in data.chunks_exact(stride) {
        pixels.extend(decode(chunk));
    }
    ImageBuffer::<Rgba<f32>, _>::from_raw(width, height, pixels)
        .map(DynamicImage::ImageRgba32F)
        .ok_or_else(|| format!("captured {format:?} image has the wrong data size"))
}

type Decode = fn(&[u8]) -> [f32; 4];

fn decoder(format: TextureFormat) -> Option<Decode> {
    Some(match format {
        TextureFormat::Rgba8Unorm => |p| [unorm8(p[0]), unorm8(p[1]), unorm8(p[2]), unorm8(p[3])],
        TextureFormat::Rgba8Snorm => |p| [snorm8(p[0]), snorm8(p[1]), snorm8(p[2]), snorm8(p[3])],
        TextureFormat::Rg8Snorm => |p| grey(snorm8(p[0])),
        TextureFormat::R8Snorm => |p| grey(snorm8(p[0])),
        TextureFormat::Rgba16Unorm => |p| {
            [
                unorm16(&p[0..2]),
                unorm16(&p[2..4]),
                unorm16(&p[4..6]),
                unorm16(&p[6..8]),
            ]
        },
        TextureFormat::Rgba16Snorm => |p| {
            [
                snorm16(&p[0..2]),
                snorm16(&p[2..4]),
                snorm16(&p[4..6]),
                snorm16(&p[6..8]),
            ]
        },
        TextureFormat::Rg16Unorm | TextureFormat::R16Unorm => |p| grey(unorm16(&p[0..2])),
        TextureFormat::Rg16Snorm | TextureFormat::R16Snorm => |p| grey(snorm16(&p[0..2])),
        TextureFormat::Rg16Float | TextureFormat::R16Float => |p| grey(float16(&p[0..2])),
        TextureFormat::Rg32Float | TextureFormat::R32Float => |p| grey(float32(&p[0..4])),
        TextureFormat::Rgb10a2Unorm => |p| {
            let bits = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
            let channel = |shift: u32, mask: u32| ((bits >> shift) & mask) as f32 / mask as f32;
            [
                channel(0, 0x3ff),
                channel(10, 0x3ff),
                channel(20, 0x3ff),
                channel(30, 0x3),
            ]
        },
        TextureFormat::Rg11b10Ufloat => |p| {
            let bits = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
            [
                small_float(bits & 0x7ff, 6),
                small_float((bits >> 11) & 0x7ff, 6),
                small_float((bits >> 22) & 0x3ff, 5),
                1.0,
            ]
        },
        _ => return None,
    })
}

fn grey(value: f32) -> [f32; 4] {
    [value, value, value, 1.0]
}

fn unorm8(byte: u8) -> f32 {
    f32::from(byte) / 255.0
}

fn snorm8(byte: u8) -> f32 {
    (f32::from(byte as i8) / 127.0).max(-1.0)
}

fn unorm16(bytes: &[u8]) -> f32 {
    f32::from(u16::from_le_bytes([bytes[0], bytes[1]])) / 65535.0
}

fn snorm16(bytes: &[u8]) -> f32 {
    (f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32767.0).max(-1.0)
}

fn float16(bytes: &[u8]) -> f32 {
    f16::from_le_bytes([bytes[0], bytes[1]]).to_f32()
}

fn float32(bytes: &[u8]) -> f32 {
    f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// An unsigned packed float with a 5-bit exponent and `mantissa_bits` mantissa.
fn small_float(bits: u32, mantissa_bits: u32) -> f32 {
    let exponent = bits >> mantissa_bits;
    let mantissa = bits & ((1 << mantissa_bits) - 1);
    let fraction = mantissa as f32 / (1 << mantissa_bits) as f32;
    match exponent {
        0 => fraction * 2f32.powi(-14),
        31 => f32::INFINITY,
        _ => (1.0 + fraction) * 2f32.powi(exponent as i32 - 15),
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::RenderAssetUsages,
        image::Image,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };

    use super::{capture_to_dynamic, small_float};

    fn one_pixel(format: TextureFormat, data: Vec<u8>) -> Image {
        Image::new(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            format,
            RenderAssetUsages::default(),
        )
    }

    #[test]
    fn linear_and_signed_formats_decode_to_rgb() {
        let cases = [
            (
                TextureFormat::Rgba8Unorm,
                vec![255, 0, 128, 255],
                [255, 0, 128],
            ),
            (TextureFormat::R8Snorm, vec![127], [255, 255, 255]),
            (
                TextureFormat::R32Float,
                0.5f32.to_le_bytes().to_vec(),
                [128, 128, 128],
            ),
            (
                TextureFormat::Rgb10a2Unorm,
                (0x3ffu32 | (0x3ff << 20)).to_le_bytes().to_vec(),
                [255, 0, 255],
            ),
        ];
        for (format, data, expected) in cases {
            let rgb = capture_to_dynamic(one_pixel(format, data))
                .unwrap()
                .to_rgb8();
            assert_eq!(rgb.get_pixel(0, 0).0, expected, "{format:?}");
        }
    }

    #[test]
    fn packed_small_floats_follow_the_ieee_layout() {
        assert_eq!(small_float(15 << 6, 6), 1.0);
        assert_eq!(small_float((14 << 6) | 32, 6), 0.75);
        assert_eq!(small_float(0, 5), 0.0);
    }

    #[test]
    fn supported_formats_still_use_bevys_conversion() {
        let image = one_pixel(TextureFormat::Rgba8UnormSrgb, vec![10, 20, 30, 255]);
        assert_eq!(
            capture_to_dynamic(image)
                .unwrap()
                .to_rgb8()
                .get_pixel(0, 0)
                .0,
            [10, 20, 30]
        );
    }
}
