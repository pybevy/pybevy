use bevy::mesh::{Mesh, MeshBuilder, Meshable};
use pyo3::{
    exceptions::{PyNotImplementedError, PyTypeError},
    prelude::*,
};

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::{
        PyAnnulus, PyCapsule2d, PyCapsule3d, PyCircle, PyCircularSector, PyCircularSegment, PyCone,
        PyConicalFrustum, PyCuboid, PyCylinder, PyEllipse, PyPlane3d, PyPolyline2d, PyPolyline3d,
        PyRectangle, PyRegularPolygon, PyRhombus, PySegment2d, PySphere, PyTetrahedron, PyTorus,
        PyTriangle2d, PyTriangle3d, validation::vertex_budget,
    },
};

#[pyclass(
    name = "Meshable",
    module = "pybevy.mesh",
    subclass,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Default)]
pub struct PyMeshable;

#[pymethods]
impl PyMeshable {
    #[new]
    pub fn new() -> Self {
        Self
    }

    pub fn mesh(&self) -> PyResult<Py<PyMeshBuilder>> {
        Err(PyNotImplementedError::new_err(
            "Meshable.mesh() must be implemented by a subclass",
        ))
    }
}

fn build_python_meshable(asset: &Bound<'_, PyAny>) -> PyResult<Mesh> {
    let builder = asset.call_method0("mesh")?;
    if !builder.is_instance_of::<PyMeshBuilder>() {
        return Err(PyTypeError::new_err(
            "Meshable.mesh() must return a MeshBuilder",
        ));
    }
    let built = builder.call_method0("build")?;
    let mesh = built
        .cast::<PyMesh>()
        .map_err(|_| PyTypeError::new_err("MeshBuilder.build() must return a Mesh"))?;
    let borrowed = mesh.borrow();
    Ok(PyMesh::as_ref(&borrowed)?.clone())
}

/// Convert a PyMeshable subclass instance directly to a Bevy Mesh.
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
    if let Ok(v) = asset.cast::<PyConicalFrustum>() {
        let py = asset.py();
        let builder = v.borrow().mesh(py)?;
        let mesh = builder.borrow(py).build(py)?;
        return Ok(PyMesh::as_ref(&mesh.borrow(py))?.clone());
    }
    if let Ok(v) = asset.cast::<PyPolyline3d>() {
        let py = asset.py();
        let builder = v.borrow().mesh(py)?;
        let mesh = builder.borrow(py).build(py)?;
        return Ok(PyMesh::as_ref(&mesh.borrow(py))?.clone());
    }
    // 2D primitives
    if let Ok(v) = asset.cast::<PyCircle>() {
        return Ok(v.borrow().storage.as_ref()?.mesh().build());
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
        let polygon = v.borrow();
        vertex_budget("RegularPolygonMeshBuilder", u128::from(polygon.0.sides))?;
        return Ok(polygon.0.mesh().build());
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
    if let Ok(v) = asset.cast::<PyPolyline2d>() {
        let py = asset.py();
        let builder = v.borrow().mesh(py)?;
        let mesh = builder.borrow(py).build(py)?;
        return Ok(PyMesh::as_ref(&mesh.borrow(py))?.clone());
    }
    build_python_meshable(asset)
}
