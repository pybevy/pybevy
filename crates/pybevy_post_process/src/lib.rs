pub mod bloom;
pub mod bloom_composite_mode;
pub mod bloom_prefilter;

use pyo3::prelude::*;

pub use bloom::PyBloom;
pub use bloom_composite_mode::PyBloomCompositeMode;
pub use bloom_prefilter::PyBloomPrefilter;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "post_process")?;
    m.add_class::<PyBloom>()?;
    m.add_class::<PyBloomCompositeMode>()?;
    m.add_class::<PyBloomPrefilter>()?;
    parent.add_submodule(&m)
}
