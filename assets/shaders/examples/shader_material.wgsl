// PyBevy equivalent of Bevy's shader_material example.
// A custom fragment shader that uses a uniform color and emissive intensity.

#import bevy_pbr::forward_io::VertexOutput

// Material uniforms — must match the fields in the @material class
// (bools and Images are excluded from the uniform struct)
struct CustomMaterial {
    color: vec4<f32>,
    intensity: f32,
}
@group(3) @binding(100) var<uniform> mat: CustomMaterial;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    return mat.color * mat.intensity;
}
