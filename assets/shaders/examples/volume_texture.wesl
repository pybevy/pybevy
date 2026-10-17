import bevy_pbr::render::forward_io::VertexOutput;
import pybevy::gen::VolumeSlice as params;

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(params::volume, params::volume_sampler,
        vec3(input.uv, params::material.slice_z));
}
