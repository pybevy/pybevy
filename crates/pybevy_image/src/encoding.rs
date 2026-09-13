use std::fmt::Display;

use bevy::image::ImageFormat;
use image::ImageFormat as EncoderFormat;
use pybevy_core::public_error;

pub fn supported_formats() -> Vec<ImageFormat> {
    EncoderFormat::all()
        .filter(EncoderFormat::writing_enabled)
        .filter_map(|format| {
            format
                .extensions_str()
                .iter()
                .find_map(|extension| ImageFormat::from_extension(extension))
        })
        .collect()
}

pub fn unsupported_format(format: impl Display) -> String {
    let names = supported_formats()
        .iter()
        .map(|format| format!("{format:?}"))
        .collect::<Vec<_>>();
    public_error::image_encoding_unsupported(format, &names)
}
