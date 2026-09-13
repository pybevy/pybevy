use bevy::{
    math::{Vec3, Vec3Swizzles},
    mesh::{Mesh, MeshVertexAttribute, PrimitiveTopology},
};
use pybevy_core::public_error;

pub(crate) fn check_triangle_list(mesh: &Mesh, operation: &str) -> Result<(), String> {
    let topology = mesh.primitive_topology();
    if topology != PrimitiveTopology::TriangleList {
        return Err(public_error::mesh_triangle_list_required(
            operation,
            format!("{topology:?}"),
        ));
    }
    Ok(())
}

pub(crate) fn check_indexed(mesh: &Mesh, operation: &str) -> Result<(), String> {
    if mesh
        .try_indices_option()
        .map_err(|error| public_error::mesh_operation_failed(operation, error))?
        .is_none()
    {
        return Err(public_error::mesh_indexed_required(operation));
    }
    Ok(())
}

/// Bound by the buffers the operation reads; an absent buffer is Bevy's own
/// missing-data error and must not be reported as an index error.
pub(crate) fn check_index_range(
    mesh: &Mesh,
    operation: &str,
    attributes: &[MeshVertexAttribute],
) -> Result<(), String> {
    let mut vertices = usize::MAX;
    for attribute in attributes {
        let Some(values) = mesh
            .try_attribute_option(attribute.id)
            .map_err(|error| public_error::mesh_operation_failed(operation, error))?
        else {
            return Ok(());
        };
        vertices = vertices.min(values.len());
    }
    check_indices_below(mesh, operation, vertices)
}

/// `duplicate_vertices` rewrites every attribute, so every buffer is indexed.
pub(crate) fn check_index_range_over_all(mesh: &Mesh, operation: &str) -> Result<(), String> {
    let Some(vertices) = mesh
        .try_attributes()
        .map_err(|error| public_error::mesh_operation_failed(operation, error))?
        .map(|(_, values)| values.len())
        .min()
    else {
        return Ok(());
    };
    check_indices_below(mesh, operation, vertices)
}

/// Bevy inverts the scale to transform normals, so two zero axes abort.
pub(crate) fn check_scale(operation: &str, scale: Vec3) -> Result<(), String> {
    if scale.yzx() * scale.zxy() == Vec3::ZERO {
        return Err(public_error::mesh_degenerate_scale(operation, scale));
    }
    Ok(())
}

fn check_indices_below(mesh: &Mesh, operation: &str, vertices: usize) -> Result<(), String> {
    let Some(indices) = mesh
        .try_indices_option()
        .map_err(|error| public_error::mesh_operation_failed(operation, error))?
    else {
        return Ok(());
    };
    if let Some(index) = indices.iter().find(|index| *index >= vertices) {
        return Err(public_error::mesh_index_out_of_range(
            operation, index, vertices,
        ));
    }
    Ok(())
}
