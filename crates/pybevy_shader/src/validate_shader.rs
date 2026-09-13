use bevy::shader::ValidateShader;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ValidateShader)]
#[pyclass(
    name = "ValidateShader",
    module = "pybevy.shader",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PyValidateShader {
    #[default]
    Disabled,
    Enabled,
}
