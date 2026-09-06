use bevy::pbr::ParallaxMappingMethod;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ParallaxMappingMethod, empty_tuple, unit_parens)]
#[pyclass(
    name = "ParallaxMappingMethod",
    module = "pybevy.pbr",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyParallaxMappingMethod {
    Occlusion(),
    Relief { max_steps: u32 },
}
