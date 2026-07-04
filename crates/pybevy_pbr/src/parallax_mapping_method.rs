use bevy::pbr::ParallaxMappingMethod;
use pyo3::prelude::*;

#[pyclass(name = "ParallaxMappingMethod", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyParallaxMappingMethod {
    Occlusion(),
    Relief { max_steps: u32 },
}

#[pymethods]
impl PyParallaxMappingMethod {
    #[new]
    pub fn new() -> Self {
        PyParallaxMappingMethod::Occlusion()
    }

    #[classattr]
    pub const OCCLUSION: Self = PyParallaxMappingMethod::Occlusion();
}

impl From<ParallaxMappingMethod> for PyParallaxMappingMethod {
    fn from(method: ParallaxMappingMethod) -> Self {
        match method {
            ParallaxMappingMethod::Occlusion => PyParallaxMappingMethod::Occlusion(),
            ParallaxMappingMethod::Relief { max_steps } => {
                PyParallaxMappingMethod::Relief { max_steps }
            }
        }
    }
}

impl From<PyParallaxMappingMethod> for ParallaxMappingMethod {
    fn from(method: PyParallaxMappingMethod) -> Self {
        match method {
            PyParallaxMappingMethod::Occlusion() => ParallaxMappingMethod::Occlusion,
            PyParallaxMappingMethod::Relief { max_steps } => {
                ParallaxMappingMethod::Relief { max_steps }
            }
        }
    }
}

impl Default for PyParallaxMappingMethod {
    fn default() -> Self {
        Self::new()
    }
}
