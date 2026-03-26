use pybevy_core::PyPlugin;
use pyo3::prelude::*;

use crate::power_preference::PyPowerPreference;

#[pyclass(name = "RenderPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct PyRenderPlugin {
    pub power_preference: Option<PyPowerPreference>,
    pub synchronous_pipeline_compilation: Option<bool>,
}

#[pymethods]
impl PyRenderPlugin {
    #[new]
    #[pyo3(signature = (power_preference = None, synchronous_pipeline_compilation = None))]
    pub fn new(
        power_preference: Option<PyPowerPreference>,
        synchronous_pipeline_compilation: Option<bool>,
    ) -> (Self, PyPlugin) {
        (
            PyRenderPlugin {
                power_preference,
                synchronous_pipeline_compilation,
            },
            PyPlugin,
        )
    }

    pub fn __repr__(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref pp) = self.power_preference {
            parts.push(format!("power_preference={pp:?}"));
        }
        if let Some(sync) = self.synchronous_pipeline_compilation {
            parts.push(format!("synchronous_pipeline_compilation={sync}"));
        }
        if parts.is_empty() {
            "RenderPlugin()".to_string()
        } else {
            format!("RenderPlugin({})", parts.join(", "))
        }
    }
}

