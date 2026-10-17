import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::SampledTexture as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(textureLoad(params::image, vec2<i32>(0, 0), 0));
}
