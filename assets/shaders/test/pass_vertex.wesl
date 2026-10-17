import bevy_pbr::render::{mesh_functions, view_transformations::position_world_to_clip};
import pybevy::gen::PassSurface as params;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput};
@else
import bevy_pbr::render::forward_io::{Vertex, VertexOutput};

@vertex
fn vertex(in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world = mesh_functions::get_world_from_local(in.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world, vec4(in.position + params::material.offset, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(in.normal, in.instance_index);
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world, in.tangent, in.instance_index);
@if(VERTEX_UVS_A)
    out.uv = in.uv;
@if(VERTEX_UVS_B)
    out.uv_b = in.uv_b;
@if(VERTEX_COLORS)
    out.color = in.color;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = in.instance_index;
@if(PREPASS_PIPELINE && MOTION_VECTOR_PREPASS) {
    let previous_world = mesh_functions::get_previous_world_from_local(in.instance_index);
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(
        previous_world, vec4(in.position + params::material.previous_offset, 1.0));
}
@if(PREPASS_PIPELINE && UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
    return out;
}
