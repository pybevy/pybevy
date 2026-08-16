use bevy::{sprite::TextureSlicer, ui::widget::NodeImageMode};
use pybevy_macros::pyenum;
use pybevy_sprite::texture_slicer::PyTextureSlicer;
use pyo3::prelude::*;

#[pyenum(NodeImageMode, empty_tuple, no_repr)]
#[pyclass(name = "NodeImageMode", frozen, eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub enum PyNodeImageMode {
    #[pyo3(name = "Auto")]
    Auto(),
    #[pyo3(name = "Stretch")]
    Stretch(),
    #[pyo3(name = "Sliced")]
    #[py_bevy(tuple)]
    Sliced {
        #[py_type(PyTextureSlicer)]
        slicer: TextureSlicer,
    },
    #[pyo3(
        name = "Tiled",
        constructor = (tile_x = true, tile_y = true, stretch_value = 1.0)
    )]
    Tiled {
        tile_x: bool,
        tile_y: bool,
        stretch_value: f32,
    },
}

#[pymethods]
impl PyNodeImageMode {
    pub fn uses_slices(&self) -> bool {
        matches!(
            self,
            PyNodeImageMode::Sliced { .. } | PyNodeImageMode::Tiled { .. }
        )
    }

    fn __repr__(&self) -> String {
        match self {
            PyNodeImageMode::Auto() => "NodeImageMode.Auto".to_string(),
            PyNodeImageMode::Stretch() => "NodeImageMode.Stretch".to_string(),
            PyNodeImageMode::Sliced { .. } => "NodeImageMode.Sliced(...)".to_string(),
            PyNodeImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => format!(
                "NodeImageMode.Tiled(tile_x={}, tile_y={}, stretch_value={})",
                tile_x, tile_y, stretch_value
            ),
        }
    }
}
