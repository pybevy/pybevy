use bevy::image::ImageFormat;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ImageFormat)]
#[pyclass(name = "ImageFormat", eq, hash, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyImageFormat {
    Bmp,
    Dds,
    Farbfeld,
    Gif,
    OpenExr,
    Hdr,
    Ico,
    Jpeg,
    Ktx2,
    Png,
    Pnm,
    Qoi,
    Tga,
    Tiff,
    WebP,
}
