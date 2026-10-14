import pybevy::gen::IndependentDepth as params;
@fragment
fn fragment() -> @location(0) vec4<f32> {
    let comparison = textureSampleCompare(params::image, params::depth_test, vec2(0.5), 0.5);
    return vec4(1.0 - comparison, comparison, 0.0, 1.0);
}
