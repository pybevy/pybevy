import bevy_pbr::render::{forward_io::VertexOutput, mesh_functions};
import pybevy::gen::LayeredTiles as params;

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let layer = (mesh_functions::get_tag(input.instance_index) + u32(params::material.layer_offset)) % 4u;
    return textureSample(params::layers, params::layers_sampler, input.uv, i32(layer));
}
