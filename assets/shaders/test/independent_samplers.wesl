import pybevy::gen::IndependentSampling as params;
@fragment
fn fragment() -> @location(0) vec4<f32> {
    if params::material.mode > 0.5 {
        return textureSample(params::image, params::soft, params::material.coord);
    }
    return textureSample(params::image, params::sharp, params::material.coord);
}
