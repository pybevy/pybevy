use bevy::shader::ValidateShader;
use pyo3::prelude::*;

#[pyclass(name = "ValidateShader", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PyValidateShader {
    #[default]
    Disabled,
    Enabled,
}

#[pymethods]
impl PyValidateShader {
    #[staticmethod]
    pub fn disabled() -> Self {
        PyValidateShader::Disabled
    }

    #[staticmethod]
    pub fn enabled() -> Self {
        PyValidateShader::Enabled
    }

    fn __repr__(&self) -> &'static str {
        match self {
            PyValidateShader::Disabled => "ValidateShader.Disabled",
            PyValidateShader::Enabled => "ValidateShader.Enabled",
        }
    }
}

impl From<PyValidateShader> for ValidateShader {
    fn from(py_validate: PyValidateShader) -> Self {
        match py_validate {
            PyValidateShader::Disabled => ValidateShader::Disabled,
            PyValidateShader::Enabled => ValidateShader::Enabled,
        }
    }
}

impl From<&ValidateShader> for PyValidateShader {
    fn from(validate: &ValidateShader) -> Self {
        match validate {
            ValidateShader::Disabled => PyValidateShader::Disabled,
            ValidateShader::Enabled => PyValidateShader::Enabled,
        }
    }
}

impl From<ValidateShader> for PyValidateShader {
    fn from(validate: ValidateShader) -> Self {
        match validate {
            ValidateShader::Disabled => PyValidateShader::Disabled,
            ValidateShader::Enabled => PyValidateShader::Enabled,
        }
    }
}
