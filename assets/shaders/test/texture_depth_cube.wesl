import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::SampledTexture as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let comparison = textureSampleCompare(params::image, params::image_sampler, vec3(1.0, 0.0, 0.0), params::material.coord.x);
    return vec4(1.0 - comparison, comparison, 0.0, 1.0);
}
