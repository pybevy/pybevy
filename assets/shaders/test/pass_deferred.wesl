import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    render::pbr_fragment::pbr_input_from_standard_material,
    deferred::functions::deferred_output,
};
import pybevy::gen::PassSurface as params;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var input = pbr_input_from_standard_material(in, is_front);
    input.material.base_color = params::material.tint;
    input.N = params::material.normal;
    input.world_normal = params::material.normal;
    return deferred_output(in, input);
}
