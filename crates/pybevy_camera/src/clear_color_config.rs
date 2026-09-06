use bevy::{camera::ClearColorConfig, color::Color};
use pybevy_color::color::{OwnedColorValue, PyColor};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ClearColorConfig, empty_tuple, no_repr)]
#[pyclass(
    name = "ClearColorConfig",
    module = "pybevy.camera",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyClearColorConfig {
    Default(),
    #[py_bevy(tuple)]
    Custom {
        #[py_type(OwnedColorValue)]
        color: Color,
    },
    #[pyo3(name = "None_")]
    None(),
}

#[pymethods]
impl PyClearColorConfig {
    fn __repr__(&self) -> PyResult<String> {
        Ok(match self {
            PyClearColorConfig::Default() => "ClearColorConfig.Default()".to_string(),
            PyClearColorConfig::Custom { color } => {
                let color: PyColor = color.0.into();
                format!("ClearColorConfig.Custom({})", color.__repr__()?)
            }
            PyClearColorConfig::None() => "ClearColorConfig.None_()".to_string(),
        })
    }
}

impl Default for PyClearColorConfig {
    fn default() -> Self {
        ClearColorConfig::default().into()
    }
}
