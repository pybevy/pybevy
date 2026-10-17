//! Typed views of values packed into `ShaderParams::data`.

use std::ops::Range;

use bevy::{
    color::LinearRgba,
    math::{UVec4, Vec2, Vec3, Vec4},
    reflect::Reflect,
    render::render_resource::ShaderType,
};
use bytemuck::Pod;

#[repr(C, align(16))]
#[derive(ShaderType, Reflect, Clone, Debug)]
pub struct UniformData {
    pub words: [UVec4; 64],
}

impl Default for UniformData {
    fn default() -> Self {
        Self {
            words: [UVec4::ZERO; 64],
        }
    }
}

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
    let bytes =
        bytemuck::cast_slice::<UVec4, u8>(&data.words).get(byte_range::<F>(float_index)?)?;
    bytemuck::try_from_bytes(bytes).ok()
}

pub fn uniform_mut<F: Pod>(data: &mut UniformData, float_index: usize) -> Option<&mut F> {
    let bytes = bytemuck::cast_slice_mut::<UVec4, u8>(&mut data.words)
        .get_mut(byte_range::<F>(float_index)?)?;
    bytemuck::try_from_bytes_mut(bytes).ok()
}

#[cfg(test)]
mod tests {
    use std::array;

    use bevy::{
        color::LinearRgba,
        math::{UVec4, Vec2, Vec3, Vec4},
    };

    use super::{UniformData, UniformIndexError, check_uniform_index, uniform_mut, uniform_ref};

    fn sequential() -> UniformData {
        UniformData {
            words: array::from_fn(|i| {
                let base = (i * 4) as f32;
                UVec4::from_array([base, base + 1.0, base + 2.0, base + 3.0].map(f32::to_bits))
            }),
        }
    }

    #[test]
    fn packed_uniforms_project_exact_floats() {
        let original = sequential();
        let mut data = sequential();
        assert_eq!(uniform_ref::<Vec2>(&data, 6), Some(&Vec2::new(6.0, 7.0)));
        assert_eq!(
            uniform_ref::<Vec3>(&data, 8),
            Some(&Vec3::new(8.0, 9.0, 10.0))
        );
        assert_eq!(
            uniform_ref::<Vec4>(&data, 252),
            Some(&Vec4::new(252.0, 253.0, 254.0, 255.0))
        );
        assert_eq!(
            uniform_ref::<LinearRgba>(&data, 4),
            Some(&LinearRgba::new(4.0, 5.0, 6.0, 7.0))
        );
        *uniform_mut::<Vec3>(&mut data, 4).unwrap() = Vec3::splat(-1.0);
        *uniform_mut::<Vec2>(&mut data, 10).unwrap() = Vec2::splat(-2.0);
        assert_eq!(
            uniform_ref::<Vec4>(&data, 4),
            Some(&Vec4::new(-1.0, -1.0, -1.0, 7.0))
        );
        assert_eq!(
            uniform_ref::<Vec4>(&data, 8),
            Some(&Vec4::new(8.0, 9.0, -2.0, -2.0))
        );
        assert_eq!(data.words[0], original.words[0]);
        assert_eq!(data.words[3..], original.words[3..]);
    }

    #[test]
    fn packed_uniforms_reject_out_of_range_and_misaligned_indices() {
        let mut data = sequential();
        assert!(uniform_ref::<Vec2>(&data, 255).is_none());
        assert!(uniform_ref::<Vec4>(&data, 253).is_none());
        assert!(uniform_mut::<Vec3>(&mut data, usize::MAX).is_none());
        if align_of::<Vec4>() == 16 {
            assert!(uniform_ref::<Vec4>(&data, 2).is_none());
        }
        assert_eq!(check_uniform_index::<Vec2>(254), Ok(()));
        assert_eq!(check_uniform_index::<LinearRgba>(252), Ok(()));
        assert_eq!(
            check_uniform_index::<Vec2>(3),
            Err(UniformIndexError::Misaligned)
        );
        assert_eq!(
            check_uniform_index::<Vec3>(6),
            Err(UniformIndexError::Misaligned)
        );
        assert_eq!(
            check_uniform_index::<Vec4>(256),
            Err(UniformIndexError::OutOfRange)
        );
        assert_eq!(
            check_uniform_index::<Vec3>(usize::MAX - 3),
            Err(UniformIndexError::OutOfRange)
        );
    }
}
