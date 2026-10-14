//! Typed views of values packed into `ShaderParams::data`.

use std::ops::Range;

use bevy::{
    color::LinearRgba,
    math::{Vec2, Vec3, Vec4},
};
use bytemuck::Pod;

pub type UniformData = [Vec4; 64];

pub const UNIFORM_FLOATS: usize = 256;

/// A value the `@material` layout packs at a WGSL-aligned float index.
pub trait PackedUniform: Pod {
    const NAME: &'static str;
    const FLOATS: usize;
    const ALIGNMENT_FLOATS: usize;
}

impl PackedUniform for Vec2 {
    const NAME: &'static str = "Vec2";
    const FLOATS: usize = 2;
    const ALIGNMENT_FLOATS: usize = 2;
}

impl PackedUniform for Vec3 {
    const NAME: &'static str = "Vec3";
    const FLOATS: usize = 3;
    const ALIGNMENT_FLOATS: usize = 4;
}

impl PackedUniform for Vec4 {
    const NAME: &'static str = "Vec4";
    const FLOATS: usize = 4;
    const ALIGNMENT_FLOATS: usize = 4;
}

impl PackedUniform for LinearRgba {
    const NAME: &'static str = "LinearRgba";
    const FLOATS: usize = 4;
    const ALIGNMENT_FLOATS: usize = 4;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniformIndexError {
    Misaligned,
    OutOfRange,
}

pub fn check_uniform_index<F: PackedUniform>(float_index: usize) -> Result<(), UniformIndexError> {
    if !float_index.is_multiple_of(F::ALIGNMENT_FLOATS) {
        return Err(UniformIndexError::Misaligned);
    }
    if float_index.saturating_add(F::FLOATS) > UNIFORM_FLOATS {
        return Err(UniformIndexError::OutOfRange);
    }
    Ok(())
}

fn byte_range<F>(float_index: usize) -> Option<Range<usize>> {
    let start = float_index.checked_mul(size_of::<f32>())?;
    Some(start..start.checked_add(size_of::<F>())?)
}

pub fn uniform_ref<F: Pod>(data: &UniformData, float_index: usize) -> Option<&F> {
    let bytes = bytemuck::cast_slice::<Vec4, u8>(data).get(byte_range::<F>(float_index)?)?;
    bytemuck::try_from_bytes(bytes).ok()
}

pub fn uniform_mut<F: Pod>(data: &mut UniformData, float_index: usize) -> Option<&mut F> {
    let bytes =
        bytemuck::cast_slice_mut::<Vec4, u8>(data).get_mut(byte_range::<F>(float_index)?)?;
    bytemuck::try_from_bytes_mut(bytes).ok()
}
