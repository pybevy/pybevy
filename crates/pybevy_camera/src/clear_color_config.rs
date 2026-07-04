use bevy::camera::ClearColorConfig;
use pybevy_color::color::PyColor;
use pyo3::prelude::*;

#[pyclass(name = "ClearColorConfig", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyClearColorConfig {
    inner: ClearColorConfig,
}

impl From<PyClearColorConfig> for ClearColorConfig {
    fn from(value: PyClearColorConfig) -> Self {
        value.inner
    }
}

impl From<ClearColorConfig> for PyClearColorConfig {
    fn from(value: ClearColorConfig) -> Self {
        PyClearColorConfig { inner: value }
    }
}

#[pymethods]
impl PyClearColorConfig {
    #[new]
    fn new() -> Self {
        PyClearColorConfig {
            inner: ClearColorConfig::default(),
        }
    }

    #[staticmethod]
    #[pyo3(name = "Default")]
    fn default_variant() -> Self {
        PyClearColorConfig {
            inner: ClearColorConfig::Default,
        }
    }

    #[staticmethod]
    #[pyo3(name = "Custom")]
    fn custom(color: PyColor) -> Self {
        PyClearColorConfig {
            inner: ClearColorConfig::Custom(color.into()),
        }
    }

    #[staticmethod]
    #[pyo3(name = "None_")]
    fn none_variant() -> Self {
        PyClearColorConfig {
            inner: ClearColorConfig::None,
        }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            ClearColorConfig::Default => "ClearColorConfig.Default()".to_string(),
            ClearColorConfig::Custom(color) => format!("ClearColorConfig.Custom({color:?})"),
            ClearColorConfig::None => "ClearColorConfig.None_()".to_string(),
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (ClearColorConfig::Default, ClearColorConfig::Default) => true,
            (ClearColorConfig::None, ClearColorConfig::None) => true,
            (ClearColorConfig::Custom(a), ClearColorConfig::Custom(b)) => a == b,
            _ => false,
        }
    }
}
