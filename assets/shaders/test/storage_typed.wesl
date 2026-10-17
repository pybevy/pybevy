import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::TypedSurface as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let paint = params::colors[params::indices[0]];
    return vec4(paint.color * paint.intensity, 1.0);
}
