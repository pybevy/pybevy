import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::IntegerGpuStorageSurface as params;
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let valid = params::records[0] == 1 && (params::material.count == 16777217 || params::material.count == 2147483647)
        && params::material.low == (-2147483647 - 1)
        && all(params::material.direction == vec3<f32>(1.0, 2.0, 3.0))
        && params::material.gain == 0.25;
    return select(vec4<f32>(1.0, 0.0, 0.0, 1.0), vec4<f32>(0.0, 1.0, 0.0, 1.0), valid);
}
