// Intentionally imports another registered material's generated bindings.
import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::WrongImportMaterial as params;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return params::material.color;
}
