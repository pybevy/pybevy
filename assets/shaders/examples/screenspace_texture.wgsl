// PyBevy equivalent of Bevy's shader_material_screenspace_texture example.
// Samples a texture using screen-space UV coordinates so the texture
// stays fixed to the viewport regardless of mesh geometry or camera angle.

#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::VertexOutput,
    utils::coords_to_viewport_uv,
}

// Texture slot 0 from @material — first Image field
@group(3) @binding(101) var texture: texture_2d<f32>;
@group(3) @binding(102) var texture_sampler: sampler;

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    let viewport_uv = coords_to_viewport_uv(mesh.position.xy, view.viewport);
    let color = textureSample(texture, texture_sampler, viewport_uv);
    return color;
}
