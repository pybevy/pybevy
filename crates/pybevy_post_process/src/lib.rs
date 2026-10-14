pub mod bloom;
pub mod bloom_composite_mode;
pub mod bloom_prefilter;
pub mod depth_of_field;
pub mod depth_of_field_mode;
pub mod lens_distortion;
pub mod motion_blur;
pub mod vignette;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        bloom::PyBloom, bloom_composite_mode::PyBloomCompositeMode,
        bloom_prefilter::PyBloomPrefilter, depth_of_field::PyDepthOfField,
        depth_of_field_mode::PyDepthOfFieldMode, lens_distortion::PyLensDistortion,
        motion_blur::PyMotionBlur, vignette::PyVignette,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "post_process")?;
    m.add_class::<bloom::PyBloom>()?;
    m.add_class::<bloom_composite_mode::PyBloomCompositeMode>()?;
    m.add_class::<bloom_prefilter::PyBloomPrefilter>()?;
    m.add_class::<vignette::PyVignette>()?;
    m.add_class::<lens_distortion::PyLensDistortion>()?;
    m.add_class::<motion_blur::PyMotionBlur>()?;
    m.add_class::<depth_of_field::PyDepthOfField>()?;
    m.add_class::<depth_of_field_mode::PyDepthOfFieldMode>()?;
    parent.add_submodule(&m)
}
