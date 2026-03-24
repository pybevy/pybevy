// Test: shader with invalid binding reference (group 0 binding 99)
// Triggers a wgpu validation error
#import bevy_pbr::forward_io::VertexOutput

struct FakeData {
    value: vec4<f32>,
}
@group(0) @binding(99) var<uniform> fake: FakeData;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return fake.value;
}
