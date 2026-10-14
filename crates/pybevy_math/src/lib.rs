pub mod affine2;
pub mod affine3a;
pub mod bvec2;
pub mod compass;
pub mod dir2;
pub mod dir3;
pub mod float_ord;
mod integer;
pub mod irect;
pub mod isometry2d;
pub mod isometry3d;
pub mod ivec2;
pub mod mat2;
pub mod mat3;
pub mod mat3a;
pub mod mat4;
pub mod quat;
pub mod range;
pub mod rect;
pub mod richcmp;
pub mod rot2;
pub mod snapshot;
pub mod urect;
pub mod uvec2;
pub mod uvec3;
pub mod vec2;
pub mod vec3;
pub mod vec3a;
pub mod vec4;

use bevy::math::Vec3;
use pybevy_core::{
    domain::{FINITE, check_for},
    public_error::{
        direction_not_unit_length, matrix_index_out_of_range, matrix_not_invertible,
        quat_axis_not_normalized,
    },
};
use pyo3::{
    PyClass,
    exceptions::{PyIndexError, PyValueError},
    prelude::*,
};

pub(crate) fn require_invertible(type_name: &str, inverse_is_finite: bool) -> PyResult<()> {
    if !inverse_is_finite {
        return Err(PyValueError::new_err(matrix_not_invertible(type_name)));
    }
    Ok(())
}

/// Reject a direction whose squared length is as far from 1 as makes Bevy's
/// `Dir` constructors panic in debug builds.
pub(crate) fn check_direction_length(operation: &str, length_squared: f32) -> PyResult<()> {
    let error = (length_squared - 1.0).abs();
    if error > 2e-2 || error.is_nan() {
        return Err(PyValueError::new_err(direction_not_unit_length(
            operation,
            length_squared.sqrt(),
        )));
    }
    Ok(())
}

/// Check `from_axis_angle` arguments of `O`: glam requires a normalized axis
/// but only asserts it under `glam_assert`.
pub(crate) fn checked_axis_angle<O: PyClass>(axis: Vec3, angle: f32) -> PyResult<(Vec3, f32)> {
    let angle = check_for::<O, _>("angle", angle, FINITE)?;
    let axis = check_for::<O, _>("axis", axis, FINITE)?;
    if !axis.is_normalized() {
        return Err(PyValueError::new_err(quat_axis_not_normalized(
            axis.length(),
        )));
    }
    Ok((axis, angle))
}

/// Resolve a Python-style column/row index against a square matrix.
///
/// Negative indices count from the end, the way every other Python sequence
/// accessor behaves; anything else is an `IndexError` naming the real range.
pub(crate) fn matrix_index(
    axis: &str,
    index: isize,
    type_name: &str,
    count: usize,
) -> PyResult<usize> {
    let resolved = if index < 0 {
        index + count as isize
    } else {
        index
    };
    if resolved < 0 || resolved >= count as isize {
        return Err(PyIndexError::new_err(matrix_index_out_of_range(
            axis, index, type_name, count,
        )));
    }
    Ok(resolved as usize)
}

pub mod prelude {
    #[doc(hidden)]
    pub use crate::{
        bvec2::PyBVec2,
        dir2::PyDir2,
        dir3::PyDir3,
        irect::PyIRect,
        isometry2d::PyIsometry2d,
        isometry3d::PyIsometry3d,
        ivec2::PyIVec2,
        mat2::PyMat2,
        mat3::PyMat3,
        mat3a::PyMat3A,
        mat4::PyMat4,
        quat::{PyEulerRot, PyQuat},
        rect::PyRect,
        rot2::PyRot2,
        urect::PyURect,
        uvec2::PyUVec2,
        uvec3::PyUVec3,
        vec2::PyVec2,
        vec3::PyVec3,
        vec3a::PyVec3A,
        vec4::PyVec4,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "math")?;
    m.add_class::<affine2::PyAffine2>()?;
    m.add_class::<mat2::PyMat2>()?;
    m.add_class::<affine3a::PyAffine3A>()?;
    m.add_class::<vec2::PyVec2>()?;
    m.add_class::<vec3::PyVec3>()?;
    m.add_class::<vec3a::PyVec3A>()?;
    m.add_class::<vec4::PyVec4>()?;
    m.add_class::<bvec2::PyBVec2>()?;
    m.add_class::<ivec2::PyIVec2>()?;
    m.add_class::<uvec2::PyUVec2>()?;
    m.add_class::<uvec3::PyUVec3>()?;
    m.add_class::<mat3::PyMat3>()?;
    m.add_class::<mat3a::PyMat3A>()?;
    m.add_class::<mat4::PyMat4>()?;
    m.add_class::<dir2::PyDir2>()?;
    m.add_class::<dir3::PyDir3>()?;
    m.add_class::<range::PyRange>()?;
    m.add_class::<rect::PyRect>()?;
    m.add_class::<irect::PyIRect>()?;
    m.add_class::<urect::PyURect>()?;
    m.add_class::<quat::PyQuat>()?;
    m.add_class::<quat::PyEulerRot>()?;
    m.add_class::<isometry2d::PyIsometry2d>()?;
    m.add_class::<isometry3d::PyIsometry3d>()?;
    m.add_class::<rot2::PyRot2>()?;
    m.add_class::<compass::PyCompassOctant>()?;
    m.add_class::<compass::PyCompassQuadrant>()?;
    m.add_class::<float_ord::PyFloatOrd>()?;
    parent.add_submodule(&m)
}
