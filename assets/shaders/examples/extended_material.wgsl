// PyBevy equivalent of Bevy's extended_material example.
// Extends PBR with a posterization/quantize effect.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

// Material uniforms — matches the @material class fields
struct QuantizeMaterial {
    quantize_steps: f32,
}
@group(3) @binding(100) var<uniform> mat: QuantizeMaterial;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Optionally modify input before lighting
    pbr_input.material.base_color.b = pbr_input.material.base_color.r;

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);

    // Posterize/quantize the lit color
    let steps = max(mat.quantize_steps, 1.0);
    out.color = vec4<f32>(floor(out.color * steps) / steps);

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    out.color = out.color * 2.0;
#endif

    return out;
}
