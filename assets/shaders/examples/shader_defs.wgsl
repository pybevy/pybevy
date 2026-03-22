// PyBevy equivalent of Bevy's shader_defs example.
// Uses #ifdef IS_RED to conditionally override the material color.
// The IS_RED define is injected by the @material decorator when
// the `is_red: bool` field is True.

#import bevy_pbr::forward_io::VertexOutput

// Auto-generated struct from @material — must match the uniform fields
// (bool fields are NOT included; they become shader defs instead)
struct CustomMaterial {
    color: vec4<f32>,
}
@group(3) @binding(100) var<uniform> material: CustomMaterial;

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
#ifdef IS_RED
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
#else
    return material.color;
#endif
}
