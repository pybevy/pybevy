// Test: output the uniform color, verifying uniform data packing.
import bevy_pbr::render::forward_io::VertexOutput;

struct UniformColorMat {
    color: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> material: UniformColorMat;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return material.color;
}
