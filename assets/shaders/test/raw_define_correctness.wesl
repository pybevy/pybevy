import bevy_pbr::render::forward_io::VertexOutput;
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(ON)
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
    @else
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
