// Test: output the uniform color — verifies uniform data packing.
#import bevy_pbr::forward_io::VertexOutput

struct UniformColorMat {
    color: vec4<f32>,
}
@group(3) @binding(100) var<uniform> material: UniformColorMat;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return material.color;
}
