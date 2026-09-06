use bevy::mesh::{Mesh, MeshBuilder, Meshable};
use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::primitives::{
    PyAnnulus, PyCapsule2d, PyCapsule3d, PyCircle, PyCircularSector, PyCircularSegment, PyCone,
    PyCuboid, PyCylinder, PyEllipse, PyPlane3d, PyRectangle, PyRegularPolygon, PyRhombus,
    PySegment2d, PySphere, PyTetrahedron, PyTorus, PyTriangle2d, PyTriangle3d,
};

#[pyclass(
    name = "Meshable",
    module = "pybevy.mesh",
    subclass,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyMeshable;

/// Convert a PyMeshable subclass instance directly to a Bevy Mesh in Rust,
/// bypassing Python method resolution entirely.
pub fn meshable_to_mesh(asset: &Bound<'_, PyAny>) -> PyResult<Mesh> {
    // 3D primitives
    if let Ok(v) = asset.cast::<PySphere>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyCuboid>() {
        return Ok(PyCuboid::as_ref(&v.borrow())?.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyCylinder>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyCone>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyCapsule3d>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyTorus>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyTetrahedron>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyTriangle3d>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyPlane3d>() {
        return Ok(PyPlane3d::as_ref(&v.borrow())?.mesh().build());
    }
    // 2D primitives
    if let Ok(v) = asset.cast::<PyCircle>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyAnnulus>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyCapsule2d>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyEllipse>() {
        return Ok(PyEllipse::as_ref(&v.borrow())?.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyRectangle>() {
        return Ok(PyRectangle::as_ref(&v.borrow())?.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyRegularPolygon>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyRhombus>() {
        return Ok(PyRhombus::as_ref(&v.borrow())?.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyTriangle2d>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyCircularSector>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PyCircularSegment>() {
        return Ok(v.borrow().0.mesh().build());
    }
    if let Ok(v) = asset.cast::<PySegment2d>() {
        return Ok(v.borrow().0.mesh().build());
    }
    let type_name = asset
        .get_type()
        .name()
        .map(|n| n.to_string())
        .unwrap_or_else(|_| "Unknown".to_string());
    Err(PyTypeError::new_err(format!(
        "Unknown meshable type: {type_name}"
    )))
}
