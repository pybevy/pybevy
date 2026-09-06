use bevy::image::ImageFormat;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageFormat)]
#[pyclass(
    name = "ImageFormat",
    module = "pybevy.image",
    eq,
    hash,
    frozen,
    from_py_object
)]
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
