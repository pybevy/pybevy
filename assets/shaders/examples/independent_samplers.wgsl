#import bevy_pbr::{forward_io::VertexOutput, mesh_functions}
#import pybevy::gen::SharedImage as params

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let sharp = textureSample(params::image, params::sharp, input.uv);
    let soft = textureSample(params::image, params::soft, input.uv);
    return select(sharp, soft, mesh_functions::get_tag(input.instance_index) == 1u);
}
