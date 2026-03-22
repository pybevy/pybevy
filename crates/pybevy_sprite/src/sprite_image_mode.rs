use bevy::sprite::SpriteImageMode;
use pyo3::prelude::*;

use crate::{scaling_mode::PySpriteScalingMode, texture_slicer::PyTextureSlicer};

#[derive(Debug, Clone, PartialEq)]
pub enum SpriteImageModeInner {
    Auto,
    Scale(PySpriteScalingMode),
    Sliced(PyTextureSlicer),
    Tiled {
        tile_x: bool,
        tile_y: bool,
        stretch_value: f32,
    },
}

#[pyclass(name = "SpriteImageMode", frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PySpriteImageMode(pub(crate) SpriteImageModeInner);

#[pymethods]
impl PySpriteImageMode {
    #[classattr]
    const AUTO: Self = PySpriteImageMode(SpriteImageModeInner::Auto);

    #[new]
    pub fn new() -> Self {
        PySpriteImageMode(SpriteImageModeInner::Auto)
    }

    #[staticmethod]
    pub fn scaled(mode: PySpriteScalingMode) -> Self {
        PySpriteImageMode(SpriteImageModeInner::Scale(mode))
    }

    #[staticmethod]
    pub fn sliced(slicer: PyTextureSlicer) -> Self {
        PySpriteImageMode(SpriteImageModeInner::Sliced(slicer))
    }

    #[staticmethod]
    #[pyo3(signature = (tile_x = true, tile_y = true, stretch_value = 1.0))]
    pub fn tiled(tile_x: bool, tile_y: bool, stretch_value: f32) -> Self {
        PySpriteImageMode(SpriteImageModeInner::Tiled {
            tile_x,
            tile_y,
            stretch_value,
        })
    }

    pub fn uses_slices(&self) -> bool {
        matches!(
            self.0,
            SpriteImageModeInner::Sliced(..) | SpriteImageModeInner::Tiled { .. }
        )
    }

    pub fn scale(&self) -> Option<PySpriteScalingMode> {
        match &self.0 {
            SpriteImageModeInner::Scale(mode) => Some(*mode),
            _ => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            SpriteImageModeInner::Auto => "SpriteImageMode.AUTO".to_string(),
            SpriteImageModeInner::Scale(mode) => format!("SpriteImageMode.scale({:?})", mode),
            SpriteImageModeInner::Sliced(_) => "SpriteImageMode.sliced(...)".to_string(),
            SpriteImageModeInner::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => {
                format!(
                    "SpriteImageMode.tiled({}, {}, {})",
                    tile_x, tile_y, stretch_value
                )
            }
        }
    }
}

impl From<SpriteImageMode> for PySpriteImageMode {
    fn from(mode: SpriteImageMode) -> Self {
        PySpriteImageMode(match mode {
            SpriteImageMode::Auto => SpriteImageModeInner::Auto,
            SpriteImageMode::Scale(scaling_mode) => {
                SpriteImageModeInner::Scale(scaling_mode.into())
            }
            SpriteImageMode::Sliced(slicer) => SpriteImageModeInner::Sliced(slicer.into()),
            SpriteImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => SpriteImageModeInner::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            },
        })
    }
}

impl From<PySpriteImageMode> for SpriteImageMode {
    fn from(mode: PySpriteImageMode) -> Self {
        match mode.0 {
            SpriteImageModeInner::Auto => SpriteImageMode::Auto,
            SpriteImageModeInner::Scale(scaling_mode) => {
                SpriteImageMode::Scale(scaling_mode.into())
            }
            SpriteImageModeInner::Sliced(slicer) => SpriteImageMode::Sliced(slicer.into()),
            SpriteImageModeInner::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => SpriteImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            },
        }
    }
}

impl Default for PySpriteImageMode {
    fn default() -> Self {
        Self::new()
    }
}
