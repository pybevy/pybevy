import bevy_pbr::render::forward_io::VertexOutput;

struct SeparateStageMat {
    offset: vec3<f32>,
    color: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> material: SeparateStageMat;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return material.color;
}
