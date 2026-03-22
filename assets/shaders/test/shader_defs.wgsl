// Test: output uniform color, but override to red if IS_RED is defined.
#import bevy_pbr::forward_io::VertexOutput

struct ShaderDefMat {
    color: vec4<f32>,
}
@group(3) @binding(100) var<uniform> material: ShaderDefMat;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef IS_RED
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
#else
    return material.color;
#endif
}
