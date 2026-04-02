pub mod alpha_mode;
pub mod atmosphere_mode;
pub mod color_grading;
pub mod color_grading_component;
pub mod face;
pub mod mip_bias;
pub mod msaa;
pub mod opaque_render_method;
pub mod plugin;
pub mod power_preference;
pub mod temporal_jitter;
pub mod unit_markers;

pub use alpha_mode::PyAlphaMode;
pub use atmosphere_mode::PyAtmosphereMode;
pub use color_grading::{PyColorGradingGlobal, PyColorGradingSection};
pub use color_grading_component::PyColorGrading;
pub use face::PyFace;
pub use mip_bias::PyMipBias;
pub use msaa::PyMsaa;
pub use opaque_render_method::PyOpaqueRenderMethod;
pub use plugin::PyRenderPlugin;
pub use power_preference::PyPowerPreference;
use pyo3::prelude::*;
pub use temporal_jitter::PyTemporalJitter;
pub use unit_markers::{PyHdr, PyNoAutomaticBatching, PyNoIndirectDrawing, PyOcclusionCulling};

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "render")?;
    m.add_class::<PyRenderPlugin>()?;
    m.add_class::<PyPowerPreference>()?;
    m.add_class::<PyAlphaMode>()?;
    m.add_class::<PyAtmosphereMode>()?;
    m.add_class::<PyFace>()?;
    m.add_class::<PyOpaqueRenderMethod>()?;
    m.add_class::<PyHdr>()?;
    m.add_class::<PyNoAutomaticBatching>()?;
    m.add_class::<PyNoIndirectDrawing>()?;
    m.add_class::<PyOcclusionCulling>()?;
    m.add_class::<PyTemporalJitter>()?;
    m.add_class::<PyMsaa>()?;
    m.add_class::<PyMipBias>()?;
    m.add_class::<PyColorGrading>()?;
    m.add_class::<PyColorGradingSection>()?;
    m.add_class::<PyColorGradingGlobal>()?;
    parent.add_submodule(&m)
}
