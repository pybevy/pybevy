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

pub use affine2::{PyAffine2, PyMat2};
pub use affine3a::PyAffine3A;
pub use bounding::{
    PyAabb2d, PyAabb3d, PyBoundingCircle, PyBoundingSphere, PyIsometry2d, PyIsometry3d,
    PyRayCast2d, PyRayCast3d,
};
pub use compass::{PyCompassOctant, PyCompassQuadrant};
pub use cubic_splines::{
    cubic_bezier::PyCubicBezier2d, cubic_cardinal_spline::PyCubicCardinalSpline2d,
    cubic_curve::PyCubicCurve2d, cubic_hermite::PyCubicHermite2d,
};
pub use dir2::PyDir2;
pub use dir3::PyDir3;
pub use easing::{PyEaseFunction, PyJumpAt};
pub use ivec2::PyIVec2;
pub use mat3::PyMat3;
pub use mat3a::PyMat3A;
pub use mat4::PyMat4;
pub use primitives::{PyArc2d, PyInfinitePlane3d, PyLine2d, PyPlane2d, PyPolygon};
use pyo3::prelude::*;
pub use quat::{PyEulerRot, PyQuat};
pub use range::PyRange;
pub use ray::{PyRay2d, PyRay3d};
pub use rect::PyRect;
pub use rot2::PyRot2;
pub use torus_kind::PyTorusKind;
pub use urect::PyURect;
pub use uvec2::PyUVec2;
pub use uvec3::PyUVec3;
pub use vec2::PyVec2;
pub use vec3::PyVec3;
pub use vec3a::PyVec3A;
pub use vec4::PyVec4;
pub use winding_order::PyWindingOrder;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "math")?;
    m.add_class::<PyAffine2>()?;
    m.add_class::<PyMat2>()?;
    m.add_class::<PyAffine3A>()?;
    m.add_class::<PyVec2>()?;
    m.add_class::<PyVec3>()?;
    m.add_class::<PyVec3A>()?;
    m.add_class::<PyVec4>()?;
    m.add_class::<PyIVec2>()?;
    m.add_class::<PyUVec2>()?;
    m.add_class::<PyUVec3>()?;
    m.add_class::<PyMat3>()?;
    m.add_class::<PyMat3A>()?;
    m.add_class::<PyMat4>()?;
    m.add_class::<PyDir2>()?;
    m.add_class::<PyDir3>()?;
    m.add_class::<PyRange>()?;
    m.add_class::<PyRect>()?;
    m.add_class::<PyURect>()?;
    m.add_class::<PyQuat>()?;
    m.add_class::<PyEulerRot>()?;
    m.add_class::<PyAabb2d>()?;
    m.add_class::<PyAabb3d>()?;
    m.add_class::<PyBoundingCircle>()?;
    m.add_class::<PyBoundingSphere>()?;
    m.add_class::<PyIsometry2d>()?;
    m.add_class::<PyIsometry3d>()?;
    m.add_class::<PyRay2d>()?;
    m.add_class::<PyRay3d>()?;
    m.add_class::<PyRayCast2d>()?;
    m.add_class::<PyRayCast3d>()?;
    m.add_class::<PyCubicCurve2d>()?;
    m.add_class::<PyCubicBezier2d>()?;
    m.add_class::<PyCubicCardinalSpline2d>()?;
    m.add_class::<PyCubicHermite2d>()?;
    m.add_class::<PyRot2>()?;
    m.add_class::<PyCompassOctant>()?;
    m.add_class::<PyCompassQuadrant>()?;
    m.add_class::<PyJumpAt>()?;
    m.add_class::<PyEaseFunction>()?;
    m.add_class::<PyTorusKind>()?;
    m.add_class::<PyWindingOrder>()?;
    // Primitives
    m.add_class::<PyLine2d>()?;
    m.add_class::<PyPlane2d>()?;
    m.add_class::<PyArc2d>()?;
    m.add_class::<PyInfinitePlane3d>()?;
    m.add_class::<PyPolygon>()?;
    parent.add_submodule(&m)
}
