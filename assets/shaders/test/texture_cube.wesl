import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::SampledTexture as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(params::image, params::image_sampler, params::material.coord.xyz);
}
