pub mod affine2;
pub mod affine3a;
pub mod bounding;
pub mod compass;
pub mod cubic_splines;
pub mod dir2;
pub mod dir3;
pub mod easing;
pub mod ivec2;
pub mod mat3;
pub mod mat3a;
pub mod mat4;
pub mod primitives;
pub mod quat;
pub mod range;
pub mod ray;
pub mod rect;
pub mod rot2;
pub mod torus_kind;
pub mod urect;
pub mod uvec2;
pub mod uvec3;
pub mod vec2;
pub mod vec3;
pub mod vec3a;
pub mod vec4;
pub mod winding_order;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        affine2::{PyAffine2, PyMat2},
        affine3a::PyAffine3A,
        compass::{PyCompassOctant, PyCompassQuadrant},
        dir2::PyDir2,
        dir3::PyDir3,
        easing::{PyEaseFunction, PyJumpAt},
        ivec2::PyIVec2,
        mat3::PyMat3,
        mat3a::PyMat3A,
        mat4::PyMat4,
        primitives::PyInfinitePlane3d,
        quat::{PyEulerRot, PyQuat},
        range::PyRange,
        ray::{PyRay2d, PyRay3d},
        rect::PyRect,
        rot2::PyRot2,
        torus_kind::PyTorusKind,
        urect::PyURect,
        uvec2::PyUVec2,
        uvec3::PyUVec3,
        vec2::PyVec2,
        vec3::PyVec3,
        vec3a::PyVec3A,
        vec4::PyVec4,
        winding_order::PyWindingOrder,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "math")?;
    m.add_class::<affine2::PyAffine2>()?;
    m.add_class::<affine2::PyMat2>()?;
    m.add_class::<affine3a::PyAffine3A>()?;
    m.add_class::<vec2::PyVec2>()?;
    m.add_class::<vec3::PyVec3>()?;
    m.add_class::<vec3a::PyVec3A>()?;
    m.add_class::<vec4::PyVec4>()?;
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
    m.add_class::<urect::PyURect>()?;
    m.add_class::<quat::PyQuat>()?;
    m.add_class::<quat::PyEulerRot>()?;
    m.add_class::<bounding::PyAabb2d>()?;
    m.add_class::<bounding::PyAabb3d>()?;
    m.add_class::<bounding::PyBoundingCircle>()?;
    m.add_class::<bounding::PyBoundingSphere>()?;
    m.add_class::<bounding::PyIsometry2d>()?;
    m.add_class::<bounding::PyIsometry3d>()?;
    m.add_class::<ray::PyRay2d>()?;
    m.add_class::<ray::PyRay3d>()?;
    m.add_class::<bounding::PyRayCast2d>()?;
    m.add_class::<bounding::PyRayCast3d>()?;
    m.add_class::<cubic_splines::cubic_curve::PyCubicCurve2d>()?;
    m.add_class::<cubic_splines::cubic_bezier::PyCubicBezier2d>()?;
    m.add_class::<cubic_splines::cubic_cardinal_spline::PyCubicCardinalSpline2d>()?;
    m.add_class::<cubic_splines::cubic_hermite::PyCubicHermite2d>()?;
    m.add_class::<rot2::PyRot2>()?;
    m.add_class::<compass::PyCompassOctant>()?;
    m.add_class::<compass::PyCompassQuadrant>()?;
    m.add_class::<easing::PyJumpAt>()?;
    m.add_class::<easing::PyEaseFunction>()?;
    m.add_class::<torus_kind::PyTorusKind>()?;
    m.add_class::<winding_order::PyWindingOrder>()?;
    // Primitives
    m.add_class::<primitives::PyLine2d>()?;
    m.add_class::<primitives::PyPlane2d>()?;
    m.add_class::<primitives::PyArc2d>()?;
    m.add_class::<primitives::PyInfinitePlane3d>()?;
    m.add_class::<primitives::PyPolygon>()?;
    parent.add_submodule(&m)
}
