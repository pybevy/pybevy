import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::SharedSurface as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return params::material.color;
}
