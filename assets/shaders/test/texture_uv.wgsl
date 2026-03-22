// Test: sample texture with mesh UVs — verifies texture binding works.
#import bevy_pbr::forward_io::VertexOutput

@group(3) @binding(101) var tex: texture_2d<f32>;
@group(3) @binding(102) var tex_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_UVS_A
    return textureSample(tex, tex_sampler, in.uv);
#else
    // Magenta fallback — should not happen for Cuboid mesh
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
#endif
}
