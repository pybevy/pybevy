import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::DigitSurface as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let pair = params::pairs3[0];
    let valid = params::material.gain2 == 0.25
        && all(params::material.tint1 == vec4<f32>(0.0, 1.0, 0.0, 1.0))
        && all(pair.p2 == vec2<f32>(0.5, 0.25))
        && pair.w1 == 0.75;
    return select(vec4<f32>(1.0, 0.0, 0.0, 1.0), vec4<f32>(0.0, 1.0, 0.0, 1.0), valid);
}
