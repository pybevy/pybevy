use bevy::sprite::{SpriteImageMode, SpriteScalingMode, TextureSlicer};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{scaling_mode::PySpriteScalingMode, texture_slicer::PyTextureSlicer};

#[pyenum(SpriteImageMode, empty_tuple, no_repr)]
#[pyclass(
    name = "SpriteImageMode",
    module = "pybevy.sprite",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PySpriteImageMode {
    Auto(),
    #[py_bevy(tuple)]
    Scale {
        #[py_type(PySpriteScalingMode)]
        mode: SpriteScalingMode,
    },
    #[py_bevy(tuple)]
    Sliced {
        #[py_type(PyTextureSlicer)]
        slicer: TextureSlicer,
    },
    #[pyo3(constructor = (tile_x = true, tile_y = true, stretch_value = 1.0))]
    Tiled {
        tile_x: bool,
        tile_y: bool,
        stretch_value: f32,
    },
}

#[pymethods]
impl PySpriteImageMode {
    pub fn uses_slices(&self) -> bool {
        matches!(self, Self::Sliced { .. } | Self::Tiled { .. })
    }

    pub fn scale(&self) -> Option<PySpriteScalingMode> {
        match self {
            Self::Scale { mode } => Some((*mode)),
            _ => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match self {
            Self::Auto() => "SpriteImageMode.Auto()".to_string(),
            Self::Scale { mode } => {
                let mode: PySpriteScalingMode = (*mode);
                format!("SpriteImageMode.Scale(mode={mode:?})")
            }
            Self::Sliced { slicer } => {
                let slicer: PyTextureSlicer = slicer.clone();
                format!("SpriteImageMode.Sliced(slicer={slicer:?})")
            }
            Self::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => format!(
                "SpriteImageMode.Tiled(tile_x={tile_x}, tile_y={tile_y}, stretch_value={stretch_value})"
            ),
        }
    }
}

impl Default for PySpriteImageMode {
    fn default() -> Self {
        SpriteImageMode::default().into()
    }
}
