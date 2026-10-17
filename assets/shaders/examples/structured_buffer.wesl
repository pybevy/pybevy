import bevy_pbr::render::{mesh_functions, view_transformations::position_world_to_clip};
import pybevy::gen::StructuredTiles as params;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vertex(input: Vertex) -> Output {
    let tag = mesh_functions::get_tag(input.instance_index);
    let tile = params::records[params::indices[tag]];
    let transform = mesh_functions::get_world_from_local(input.instance_index);
    let position = mesh_functions::mesh_position_local_to_world(transform,
        vec4(input.position * tile.scale + tile.offset, 1.0));
    var out: Output;
    out.position = position_world_to_clip(position.xyz);
    let light = 0.35 + 0.65 * max(dot(input.normal, normalize(vec3(1.0, 2.0, 3.0))), 0.0);
    out.color = vec4(tile.color.rgb * light, 1.0);
    return out;
}

@fragment
fn fragment(input: Output) -> @location(0) vec4<f32> {
    return input.color;
}
