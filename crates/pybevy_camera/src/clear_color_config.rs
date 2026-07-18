use bevy::camera::ClearColorConfig;
use pybevy_color::color::PyColor;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ClearColorConfig, manual)]
#[pyclass(name = "ClearColorConfig", frozen, eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyClearColorConfig {
    Default(),
    Custom {
        color: PyColor,
    },
    #[pyo3(name = "None_")]
    None(),
}

impl From<PyClearColorConfig> for ClearColorConfig {
    fn from(value: PyClearColorConfig) -> Self {
        match value {
            PyClearColorConfig::Default() => ClearColorConfig::Default,
            PyClearColorConfig::Custom { color } => ClearColorConfig::Custom(color.into()),
            PyClearColorConfig::None() => ClearColorConfig::None,
        }
    }
}

impl From<ClearColorConfig> for PyClearColorConfig {
    fn from(value: ClearColorConfig) -> Self {
        match value {
            ClearColorConfig::Default => PyClearColorConfig::Default(),
            ClearColorConfig::Custom(color) => PyClearColorConfig::Custom {
                color: color.into(),
            },
            ClearColorConfig::None => PyClearColorConfig::None(),
        }
    }
}

#[pymethods]
impl PyClearColorConfig {
    fn __repr__(&self) -> String {
        match self {
            PyClearColorConfig::Default() => "ClearColorConfig.Default()".to_string(),
            PyClearColorConfig::Custom { color } => {
                format!("ClearColorConfig.Custom({})", color.__repr__())
            }
            PyClearColorConfig::None() => "ClearColorConfig.None_()".to_string(),
        }
    }
}

impl Default for PyClearColorConfig {
    fn default() -> Self {
        ClearColorConfig::default().into()
    }
}
