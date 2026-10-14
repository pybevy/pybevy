import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::TextureStorage as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(params::image, params::image_sampler, vec3(1.0, 0.0, 0.0)) * params::values[0];
}
