import bevy_pbr::{prepass::io as prepass_io, render::pbr_prepass_functions::calculate_motion_vector};
import pybevy::gen::PassSurface as params;

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(in: prepass_io::VertexOutput) -> prepass_io::FragmentOutput {
    @if(CUTOUT_ENABLED)
    if in.uv.x < 0.5 { discard; }
    var out: prepass_io::FragmentOutput;
    @if(NORMAL_PREPASS)
    out.normal = vec4(params::material.normal * 0.5 + vec3(0.5), 1.0);
    @if(MOTION_VECTOR_PREPASS)
    out.motion_vector = calculate_motion_vector(in.world_position, in.previous_world_position);
    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
    return out;
}
@else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
    @if(CUTOUT_ENABLED)
    if in.uv.x < 0.5 { discard; }
}
