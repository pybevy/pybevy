import bevy_pbr::{render::forward_io::VertexOutput, prepass::utils as prepass_utils};
import pybevy::gen::PassSurface as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(CUTOUT_ENABLED)
    if in.uv.x < 0.5 { discard; }
    @if(INSPECT_DEPTH) {
        let matches = abs(prepass_utils::prepass_depth(in.position, 0u) - in.position.z) < 0.00001;
        return select(vec4(1.0, 0.0, 0.0, 1.0), vec4(0.0, 1.0, 0.0, 1.0), matches);
    } @elif(INSPECT_NORMAL) {
        let matches = prepass_utils::prepass_normal(in.position, 0u).x > 0.9;
        return select(vec4(1.0, 0.0, 0.0, 1.0), vec4(0.0, 1.0, 0.0, 1.0), matches);
    } @elif(INSPECT_MOTION) {
        let moving = length(prepass_utils::prepass_motion_vector(in.position, 0u)) > 0.00001;
        return select(vec4(1.0, 0.0, 0.0, 1.0), vec4(0.0, 1.0, 0.0, 1.0), moving);
    } @else {
        return params::material.tint;
    }
}
