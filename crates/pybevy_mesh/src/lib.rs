pub mod indices;
pub mod mesh;
pub mod mesh2d;
pub mod mesh3d;
pub mod mesh_builder;
pub mod mesh_material_2d;
pub mod mesh_material_3d;
pub mod mesh_tag;
pub mod meshable;
pub mod morph_weights;
pub mod plugin;
pub mod primitive_topology;
pub mod primitives;
pub mod skinned_mesh;
pub mod skinned_mesh_inverse_bindposes;
pub mod sphere_kind;
pub mod vertex_attribute;

use bevy::{
    mesh::{
        Mesh2d, Mesh3d, MeshTag,
        morph::MorphWeights,
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    },
    pbr::{MeshMaterial3d, StandardMaterial},
    sprite_render::{ColorMaterial, MeshMaterial2d},
};
pub use indices::{PyIndices, PyIndicesIterator};
pub use mesh::{MeshAttributeContext, MeshAttributeContextMut, PyMesh};
pub use mesh_builder::PyMeshBuilder;
pub use mesh_material_2d::PyMeshMaterial2d;
pub use mesh_material_3d::PyMeshMaterial3d;
pub use mesh_tag::PyMeshTag;
pub use mesh2d::PyMesh2d;
pub use mesh3d::PyMesh3d;
pub use meshable::{PyMeshable, meshable_to_mesh};
pub use morph_weights::PyMorphWeights;
pub use plugin::PyMeshPlugin;
pub use primitive_topology::PyPrimitiveTopology;
pub use primitives::{
    PyAnnulus, PyCapsule2d, PyCapsule3d, PyCircle, PyCircularSector, PyCircularSegment, PyCone,
    PyCuboid, PyCylinder, PyEllipse, PyPlane3d, PyRectangle, PyRegularPolygon, PyRhombus,
    PySegment2d, PySphere, PyTetrahedron, PyTorus, PyTriangle2d, PyTriangle3d,
};
pub use primitives::{
    PyAnnulusMeshBuilder, PyCapsule2dMeshBuilder, PyCapsule3dMeshBuilder, PyCircleMeshBuilder,
    PyCircularSectorMeshBuilder, PyCircularSegmentMeshBuilder, PyConeMeshBuilder,
    PyCuboidMeshBuilder, PyCylinderMeshBuilder, PyEllipseMeshBuilder, PyPlaneMeshBuilder,
    PyRectangleMeshBuilder, PyRegularPolygonMeshBuilder, PyRhombusMeshBuilder,
    PySegment2dMeshBuilder, PySphereMeshBuilder, PyTetrahedronMeshBuilder, PyTorusMeshBuilder,
    PyTriangle2dMeshBuilder, PyTriangle3dMeshBuilder,
};
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{asset_bridge, component_bridge, handle_bridge, plugin_bridge};
use pyo3::prelude::*;
pub use skinned_mesh::PySkinnedMesh;
pub use skinned_mesh_inverse_bindposes::PySkinnedMeshInverseBindposes;
pub use sphere_kind::PySphereKind;
pub use vertex_attribute::{PyMeshVertexAttribute, PyVertexAttributeValues};

component_bridge!(MeshTag, PyMeshTag, view_fields = [0 as value]);
component_bridge!(MorphWeights, PyMorphWeights);
component_bridge!(SkinnedMesh, PySkinnedMesh);

handle_bridge!(Mesh3d, PyMesh3d);
handle_bridge!(Mesh2d, PyMesh2d);
handle_bridge!(
    MeshMaterial3d::<StandardMaterial>,
    PyMeshMaterial3d,
    "MeshMaterial3d"
);
handle_bridge!(
    MeshMaterial2d::<ColorMaterial>,
    PyMeshMaterial2d,
    "MeshMaterial2d"
);

plugin_bridge!(PyMeshPlugin, bevy::mesh::MeshPlugin);

asset_bridge!(
    SkinnedMeshInverseBindposes,
    PySkinnedMeshInverseBindposes,
    not_loadable
);
pub fn register_mesh_bridges() {
    global_registry::register_component_bridge(MeshTagBridge);
    register_mesh_tag_batch();
    global_registry::register_component_bridge(MorphWeightsBridge);
    global_registry::register_component_bridge(SkinnedMeshBridge);
    global_registry::register_component_bridge(Mesh3dBridge);
    global_registry::register_component_bridge(Mesh2dBridge);
    global_registry::register_component_bridge(MeshMaterial3dBridge);
    global_registry::register_component_bridge(MeshMaterial2dBridge);

    plugin_registry::register_plugin_bridge(MeshPluginBridge);
    global_registry::register_asset_bridge(SkinnedMeshInverseBindposesBridge);
}
pub fn add_mesh_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_mesh_bridges();

    m.add_class::<PyMeshPlugin>()?;
    m.add_class::<PyIndices>()?;
    m.add_class::<PyIndicesIterator>()?;
    m.add_class::<PyMesh>()?;
    m.add_class::<PyMesh2d>()?;
    m.add_class::<PyMesh3d>()?;
    m.add_class::<PyMeshMaterial2d>()?;
    m.add_class::<PyMeshMaterial3d>()?;
    m.add_class::<PyMeshTag>()?;
    m.add_class::<PyMorphWeights>()?;
    m.add_class::<PySkinnedMesh>()?;
    m.add_class::<PySkinnedMeshInverseBindposes>()?;
    m.add_class::<PyMeshVertexAttribute>()?;
    m.add_class::<PyPrimitiveTopology>()?;
    m.add_class::<PySphereKind>()?;
    m.add_class::<PyVertexAttributeValues>()?;
    m.add_class::<MeshAttributeContext>()?;
    m.add_class::<MeshAttributeContextMut>()?;
    m.add_class::<PyMeshable>()?;

    m.add_class::<PyMeshBuilder>()?;
    m.add_class::<PyAnnulusMeshBuilder>()?;
    m.add_class::<PyCapsule2dMeshBuilder>()?;
    m.add_class::<PyCapsule3dMeshBuilder>()?;
    m.add_class::<PyCircleMeshBuilder>()?;
    m.add_class::<PyConeMeshBuilder>()?;
    m.add_class::<PyCuboidMeshBuilder>()?;
    m.add_class::<PyCylinderMeshBuilder>()?;
    m.add_class::<PyEllipseMeshBuilder>()?;
    m.add_class::<PyPlaneMeshBuilder>()?;
    m.add_class::<PyRectangleMeshBuilder>()?;
    m.add_class::<PyRegularPolygonMeshBuilder>()?;
    m.add_class::<PyRhombusMeshBuilder>()?;
    m.add_class::<PySphereMeshBuilder>()?;
    m.add_class::<PyTetrahedronMeshBuilder>()?;
    m.add_class::<PyTorusMeshBuilder>()?;
    m.add_class::<PyTriangle2dMeshBuilder>()?;
    m.add_class::<PyTriangle3dMeshBuilder>()?;
    m.add_class::<PyCircularSectorMeshBuilder>()?;
    m.add_class::<PyCircularSegmentMeshBuilder>()?;
    m.add_class::<PySegment2dMeshBuilder>()?;

    m.add_class::<PyAnnulus>()?;
    m.add_class::<PyCapsule2d>()?;
    m.add_class::<PyCircle>()?;
    m.add_class::<PyEllipse>()?;
    m.add_class::<PyRectangle>()?;
    m.add_class::<PyRegularPolygon>()?;
    m.add_class::<PyRhombus>()?;
    m.add_class::<PyTriangle2d>()?;
    m.add_class::<PyCircularSector>()?;
    m.add_class::<PyCircularSegment>()?;
    m.add_class::<PySegment2d>()?;

    m.add_class::<PyCapsule3d>()?;
    m.add_class::<PyCone>()?;
    m.add_class::<PyCuboid>()?;
    m.add_class::<PyCylinder>()?;
    m.add_class::<PyPlane3d>()?;
    m.add_class::<PySphere>()?;
    m.add_class::<PyTetrahedron>()?;
    m.add_class::<PyTorus>()?;
    m.add_class::<PyTriangle3d>()?;
    Ok(())
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
    // 2D
    m.add_class::<PyAnnulus>()?;
    m.add_class::<PyCapsule2d>()?;
    m.add_class::<PyCircle>()?;
    m.add_class::<PyEllipse>()?;
    m.add_class::<PyRectangle>()?;
    m.add_class::<PyRegularPolygon>()?;
    m.add_class::<PyRhombus>()?;
    m.add_class::<PyTriangle2d>()?;
    m.add_class::<PyCircularSector>()?;
    m.add_class::<PyCircularSegment>()?;
    m.add_class::<PySegment2d>()?;
    // 3D
    m.add_class::<PyCapsule3d>()?;
    m.add_class::<PyCone>()?;
    m.add_class::<PyCuboid>()?;
    m.add_class::<PyCylinder>()?;
    m.add_class::<PyPlane3d>()?;
    m.add_class::<PySphere>()?;
    m.add_class::<PyTetrahedron>()?;
    m.add_class::<PyTorus>()?;
    m.add_class::<PyTriangle3d>()?;
    Ok(())
}
pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "mesh")?;
    add_mesh_classes(&m)?;
    parent.add_submodule(&m)
}
