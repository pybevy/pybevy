// PyBevy equivalent of Bevy's extended_material example.
// Extends PBR with a posterization/quantize effect.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

// Material uniforms matching the @material class fields
struct QuantizeMaterial {
    quantize_steps: f32,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> mat: QuantizeMaterial;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Optionally modify input before lighting
    pbr_input.material.base_color.b = pbr_input.material.base_color.r;

    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );

    var out: FragmentOutput;
@if(PREPASS_PIPELINE) {
    out = deferred_output(in, pbr_input);
} @else {
    out.color = apply_pbr_lighting(pbr_input);

    // Posterize/quantize the lit color
    let steps = max(mat.quantize_steps, 1.0);
    out.color = vec4<f32>(floor(out.color * steps) / steps);

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    out.color = out.color * 2.0;
}

    return out;
}
