import bevy_pbr::render::forward_io::VertexOutput;

struct RuntimeMutationMat {
    color: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> material: RuntimeMutationMat;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var tex_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(IS_RED)
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    @elif(USE_TEXTURE)
    return textureSample(tex, tex_sampler, in.uv);
    @else
    return material.color;
}
