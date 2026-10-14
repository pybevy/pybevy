import bevy_pbr::{render::forward_io::VertexOutput, prepass::utils::prepass_depth};
import pybevy::gen::BufferedSurface as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(INSPECT_DEPTH) {
        let matches = abs(prepass_depth(in.position, 0u) - in.position.z) < 0.0001;
        return select(vec4(1.0, 0.0, 0.0, 1.0), vec4(0.0, 1.0, 0.0, 1.0), matches);
    } @else {
        return params::values[0] * params::material.tint
            * textureSample(params::albedo, params::albedo_sampler, in.uv);
    }
}
