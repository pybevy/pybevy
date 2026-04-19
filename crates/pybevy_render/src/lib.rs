pub mod alpha_mode;
pub mod color_grading;
pub mod color_grading_component;
pub mod face;
pub mod mip_bias;
pub mod msaa;
pub mod plugin;
pub mod power_preference;
pub mod temporal_jitter;
pub mod unit_markers;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        alpha_mode::PyAlphaMode,
        color_grading::{PyColorGradingGlobal, PyColorGradingSection},
        color_grading_component::PyColorGrading,
        mip_bias::PyMipBias,
        msaa::PyMsaa,
        plugin::PyRenderPlugin,
        temporal_jitter::PyTemporalJitter,
        unit_markers::PyHdr,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "render")?;
    m.add_class::<plugin::PyRenderPlugin>()?;
    m.add_class::<power_preference::PyPowerPreference>()?;
    m.add_class::<alpha_mode::PyAlphaMode>()?;
    m.add_class::<face::PyFace>()?;
    m.add_class::<unit_markers::PyHdr>()?;
    m.add_class::<unit_markers::PyNoAutomaticBatching>()?;
    m.add_class::<unit_markers::PyNoIndirectDrawing>()?;
    m.add_class::<unit_markers::PyOcclusionCulling>()?;
    m.add_class::<temporal_jitter::PyTemporalJitter>()?;
    m.add_class::<msaa::PyMsaa>()?;
    m.add_class::<mip_bias::PyMipBias>()?;
    m.add_class::<color_grading_component::PyColorGrading>()?;
    m.add_class::<color_grading::PyColorGradingSection>()?;
    m.add_class::<color_grading::PyColorGradingGlobal>()?;
    parent.add_submodule(&m)
}
