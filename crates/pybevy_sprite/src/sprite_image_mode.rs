use bevy::sprite::SpriteImageMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{scaling_mode::PySpriteScalingMode, texture_slicer::PyTextureSlicer};

#[pyenum(SpriteImageMode, manual)]
#[pyclass(name = "SpriteImageMode", frozen, eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PySpriteImageMode {
    Auto(),
    Scale {
        mode: PySpriteScalingMode,
    },
    Sliced {
        slicer: PyTextureSlicer,
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
            Self::Scale { mode } => Some(*mode),
            _ => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match self {
            Self::Auto() => "SpriteImageMode.Auto()".to_string(),
            Self::Scale { mode } => format!("SpriteImageMode.Scale(mode={mode:?})"),
            Self::Sliced { slicer } => format!("SpriteImageMode.Sliced(slicer={slicer:?})"),
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

impl From<SpriteImageMode> for PySpriteImageMode {
    fn from(mode: SpriteImageMode) -> Self {
        match mode {
            SpriteImageMode::Auto => Self::Auto(),
            SpriteImageMode::Scale(mode) => Self::Scale { mode: mode.into() },
            SpriteImageMode::Sliced(slicer) => Self::Sliced {
                slicer: slicer.into(),
            },
            SpriteImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => Self::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            },
        }
    }
}

impl From<PySpriteImageMode> for SpriteImageMode {
    fn from(mode: PySpriteImageMode) -> Self {
        match mode {
            PySpriteImageMode::Auto() => Self::Auto,
            PySpriteImageMode::Scale { mode } => Self::Scale(mode.into()),
            PySpriteImageMode::Sliced { slicer } => Self::Sliced(slicer.into()),
            PySpriteImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => Self::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            },
        }
    }
}

impl Default for PySpriteImageMode {
    fn default() -> Self {
        SpriteImageMode::default().into()
    }
}
