import bevy_pbr::render::forward_io::VertexOutput;

struct MixedUniformMat {
    scalar: f32,
    offset: vec3<f32>,
    count: i32,
    uv_scale: vec2<f32>,
    tint: vec4<f32>,
    color: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> material: MixedUniformMat;

fn near(actual: f32, expected: f32) -> bool {
    return abs(actual - expected) < 0.0001;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let valid = near(material.scalar, 0.125)
        && all(abs(material.offset - vec3<f32>(0.75, 1.0, 1.25)) < vec3<f32>(0.0001))
        && material.count == 2
        && all(abs(material.uv_scale - vec2<f32>(0.25, 0.5)) < vec2<f32>(0.0001))
        && all(abs(material.tint - vec4<f32>(1.5, 1.75, 2.0, 2.25)) < vec4<f32>(0.0001))
        && all(abs(material.color - vec4<f32>(0.1, 0.2, 0.3, 0.4)) < vec4<f32>(0.0001));
    if valid {
        return vec4<f32>(0.0, 1.0, 0.0, 1.0);
    }
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
