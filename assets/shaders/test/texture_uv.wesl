// Test: sample texture with mesh UVs, verifying the texture binding works.
import bevy_pbr::render::forward_io::VertexOutput;

@group(constants::MATERIAL_BIND_GROUP) @binding(101) var tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var tex_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(VERTEX_UVS_A)
    return textureSample(tex, tex_sampler, in.uv);
    // Magenta fallback, unexpected for a Cuboid mesh
    @else
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}
