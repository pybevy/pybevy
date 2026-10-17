import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::TextureSlotsMat as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.uv.x < 0.25 {
        return textureSample(params::tex_0, params::tex_0_sampler, in.uv);
    }
    if in.uv.x < 0.5 {
        return textureSample(params::tex_1, params::tex_1_sampler, in.uv);
    }
    if in.uv.x < 0.75 {
        return textureSample(params::tex_2, params::tex_2_sampler, in.uv);
    }
    return textureSample(params::tex_3, params::tex_3_sampler, in.uv);
}
