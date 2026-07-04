pub mod bloom;
pub mod bloom_composite_mode;
pub mod bloom_prefilter;
pub mod lens_distortion;
pub mod vignette;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        bloom::PyBloom, bloom_composite_mode::PyBloomCompositeMode,
        bloom_prefilter::PyBloomPrefilter, lens_distortion::PyLensDistortion, vignette::PyVignette,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "post_process")?;
    m.add_class::<bloom::PyBloom>()?;
    m.add_class::<bloom_composite_mode::PyBloomCompositeMode>()?;
    m.add_class::<bloom_prefilter::PyBloomPrefilter>()?;
    m.add_class::<vignette::PyVignette>()?;
    m.add_class::<lens_distortion::PyLensDistortion>()?;
    parent.add_submodule(&m)
}
