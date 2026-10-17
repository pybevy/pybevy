import bevy_pbr::render::{
    forward_io::VertexOutput,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    let pbr = pbr_input_from_standard_material(in, is_front);
    return alpha_discard(pbr.material.flags, pbr.material.alpha_cutoff, pbr.material.base_color);
}
