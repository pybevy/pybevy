pub mod indices;
pub mod mesh;
pub mod mesh2d;
pub mod mesh3d;
pub mod mesh_builder;
pub mod mesh_material_2d;
pub mod mesh_material_3d;
pub mod mesh_morph_weights;
pub mod mesh_tag;
pub mod meshable;
pub mod morph_weights;
pub mod plugin;
pub mod primitive_topology;
pub mod primitives;
pub mod skinned_mesh;
pub mod skinned_mesh_inverse_bindposes;
pub mod sphere_kind;
pub mod uv_channel;
pub mod vertex_attribute;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        mesh::PyMesh, mesh_builder::PyMeshBuilder, mesh_material_2d::PyMeshMaterial2d,
        mesh_material_3d::PyMeshMaterial3d, mesh2d::PyMesh2d, mesh3d::PyMesh3d,
        meshable::PyMeshable, morph_weights::PyMorphWeights, plugin::PyMeshPlugin,
        primitive_topology::PyPrimitiveTopology,
    };
}

/// Register meshable primitives into the `math` Python module.
///
/// These types (Circle, Sphere, CircularSector, etc.) are Bevy math primitives
/// that also implement `Meshable`. In Bevy, the type lives in `bevy_math` and
/// the `impl Meshable` is in `bevy_mesh`. PyO3 can't do this — the base class
/// (`extends = PyMeshable`) must be on the struct definition, so the wrapper
/// structs live here in `pybevy_mesh`. This function injects them into the
/// `math` module so users can `from pybevy.math import Circle`.
///
/// Called from `src/lib.rs` after `pybevy_math::add_module()` creates the
/// `math` submodule.
pub fn add_math_primitives(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<primitives::PyAnnulus>()?;
    m.add_class::<primitives::PyCapsule2d>()?;
    m.add_class::<primitives::PyCircle>()?;
    m.add_class::<primitives::PyEllipse>()?;
    m.add_class::<primitives::PyRectangle>()?;
    m.add_class::<primitives::PyRegularPolygon>()?;
    m.add_class::<primitives::PyRhombus>()?;
    m.add_class::<primitives::PyTriangle2d>()?;
    m.add_class::<primitives::PyCircularSector>()?;
    m.add_class::<primitives::PyCircularSegment>()?;
    m.add_class::<primitives::PySegment2d>()?;
    m.add_class::<primitives::PyCapsule3d>()?;
    m.add_class::<primitives::PyCone>()?;
    m.add_class::<primitives::PyCuboid>()?;
    m.add_class::<primitives::PyCylinder>()?;
    m.add_class::<primitives::PyPlane3d>()?;
    m.add_class::<primitives::PySphere>()?;
    m.add_class::<primitives::PyTetrahedron>()?;
    m.add_class::<primitives::PyTorus>()?;
    m.add_class::<primitives::PyTriangle3d>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "mesh")?;
    m.add_class::<plugin::PyMeshPlugin>()?;
    m.add_class::<indices::PyIndices>()?;
    m.add_class::<indices::PyIndicesIterator>()?;
    m.add_class::<mesh::PyMesh>()?;
    m.add_class::<mesh2d::PyMesh2d>()?;
    m.add_class::<mesh3d::PyMesh3d>()?;
    m.add_class::<mesh_material_2d::PyMeshMaterial2d>()?;
    m.add_class::<mesh_material_3d::PyMeshMaterial3d>()?;
    m.add_class::<mesh_morph_weights::PyMeshMorphWeights>()?;
    mesh_morph_weights::register_mesh_morph_weights_variants(&m)?;
    m.add_class::<mesh_tag::PyMeshTag>()?;
    m.add_class::<morph_weights::PyMorphWeights>()?;
    m.add_class::<skinned_mesh::PySkinnedMesh>()?;
    m.add_class::<skinned_mesh_inverse_bindposes::PySkinnedMeshInverseBindposes>()?;
    m.add_class::<vertex_attribute::PyMeshVertexAttribute>()?;
    m.add_class::<primitive_topology::PyPrimitiveTopology>()?;
    m.add_class::<sphere_kind::PySphereKind>()?;
    m.add_class::<uv_channel::PyUvChannel>()?;
    m.add_class::<vertex_attribute::PyVertexAttributeValues>()?;
    m.add_class::<mesh::MeshBoundedContextMut>()?;
    m.add_class::<meshable::PyMeshable>()?;

    m.add_class::<mesh_builder::PyMeshBuilder>()?;
    m.add_class::<primitives::PyAnnulusMeshBuilder>()?;
    m.add_class::<primitives::PyCapsule2dMeshBuilder>()?;
    m.add_class::<primitives::PyCapsule3dMeshBuilder>()?;
    m.add_class::<primitives::PyCircleMeshBuilder>()?;
    m.add_class::<primitives::PyConeMeshBuilder>()?;
    m.add_class::<primitives::PyCuboidMeshBuilder>()?;
    m.add_class::<primitives::PyCylinderMeshBuilder>()?;
    m.add_class::<primitives::PyEllipseMeshBuilder>()?;
    m.add_class::<primitives::PyPlaneMeshBuilder>()?;
    m.add_class::<primitives::PyRectangleMeshBuilder>()?;
    m.add_class::<primitives::PyRegularPolygonMeshBuilder>()?;
    m.add_class::<primitives::PyRhombusMeshBuilder>()?;
    m.add_class::<primitives::PySphereMeshBuilder>()?;
    m.add_class::<primitives::PyTetrahedronMeshBuilder>()?;
    m.add_class::<primitives::PyTorusMeshBuilder>()?;
    m.add_class::<primitives::PyTriangle2dMeshBuilder>()?;
    m.add_class::<primitives::PyTriangle3dMeshBuilder>()?;
    m.add_class::<primitives::PyCircularSectorMeshBuilder>()?;
    m.add_class::<primitives::PyCircularSegmentMeshBuilder>()?;
    m.add_class::<primitives::PySegment2dMeshBuilder>()?;

    m.add_class::<primitives::PyAnnulus>()?;
    m.add_class::<primitives::PyCapsule2d>()?;
    m.add_class::<primitives::PyCircle>()?;
    m.add_class::<primitives::PyEllipse>()?;
    m.add_class::<primitives::PyRectangle>()?;
    m.add_class::<primitives::PyRegularPolygon>()?;
    m.add_class::<primitives::PyRhombus>()?;
    m.add_class::<primitives::PyTriangle2d>()?;
    m.add_class::<primitives::PyCircularSector>()?;
    m.add_class::<primitives::PyCircularSegment>()?;
    m.add_class::<primitives::PySegment2d>()?;

    m.add_class::<primitives::PyCapsule3d>()?;
    m.add_class::<primitives::PyCone>()?;
    m.add_class::<primitives::PyCuboid>()?;
    m.add_class::<primitives::PyCylinder>()?;
    m.add_class::<primitives::PyPlane3d>()?;
    m.add_class::<primitives::PySphere>()?;
    m.add_class::<primitives::PyTetrahedron>()?;
    m.add_class::<primitives::PyTorus>()?;
    m.add_class::<primitives::PyTriangle3d>()?;
    parent.add_submodule(&m)
}
