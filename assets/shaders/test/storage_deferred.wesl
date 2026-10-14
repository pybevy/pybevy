import bevy_pbr::{prepass::io as prepass_io, render::pbr_fragment::pbr_input_from_standard_material, deferred::functions::deferred_output};
import pybevy::gen::BufferedSurface as params;

@fragment
fn fragment(in: prepass_io::VertexOutput, @builtin(front_facing) front: bool) -> prepass_io::FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, front);
    pbr.material.base_color = params::values[0] * params::material.tint;
    return deferred_output(in, pbr);
}
