import bevy_pbr::render::{forward_io::VertexOutput, mesh_view_bindings::view};
import pybevy::gen::CubeReflection as params;

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let incident = normalize(input.world_position.xyz - view.world_position);
    let reflected = reflect(incident, normalize(input.world_normal));
    let angle = params::material.angle;
    let direction = vec3(
        cos(angle) * reflected.x + sin(angle) * reflected.z,
        reflected.y,
        -sin(angle) * reflected.x + cos(angle) * reflected.z);
    return textureSample(params::environment, params::environment_sampler, direction);
}
