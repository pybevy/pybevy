pub mod bounding;
pub mod primitives;
pub mod ray;
pub mod torus_kind;
pub mod winding_order;

use pyo3::prelude::*;

pub mod prelude {
    #[doc(hidden)]
    pub use crate::{
        primitives::{
            PyArc2d, PyHalfSpace, PyInfinitePlane3d, PyLine2d, PyLine3d, PyPlane2d, PyPolygon,
            PyViewFrustum,
        },
        ray::{PyRay2d, PyRay3d},
        torus_kind::PyTorusKind,
        winding_order::PyWindingOrder,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "shape")?;
    m.add_class::<bounding::PyAabb2d>()?;
    m.add_class::<bounding::PyAabb3d>()?;
    m.add_class::<bounding::PyAabbCast2d>()?;
    m.add_class::<bounding::PyAabbCast3d>()?;
    m.add_class::<bounding::PyBoundingCircle>()?;
    m.add_class::<bounding::PyBoundingCircleCast>()?;
    m.add_class::<bounding::PyBoundingSphere>()?;
    m.add_class::<bounding::PyBoundingSphereCast>()?;
    m.add_class::<ray::PyRay2d>()?;
    m.add_class::<ray::PyRay3d>()?;
    m.add_class::<bounding::PyRayCast2d>()?;
    m.add_class::<bounding::PyRayCast3d>()?;
    m.add_class::<torus_kind::PyTorusKind>()?;
    m.add_class::<winding_order::PyWindingOrder>()?;
    m.add_class::<primitives::PyHalfSpace>()?;
    m.add_class::<primitives::half_spaces::PyHalfSpaces>()?;
    m.add_class::<primitives::PyViewFrustum>()?;
    m.add_class::<primitives::PyLine2d>()?;
    m.add_class::<primitives::PyLine3d>()?;
    m.add_class::<primitives::PyPlane2d>()?;
    m.add_class::<primitives::PyArc2d>()?;
    m.add_class::<primitives::PyInfinitePlane3d>()?;
    m.add_class::<primitives::PyPolygon>()?;
    parent.add_submodule(&m)
}
