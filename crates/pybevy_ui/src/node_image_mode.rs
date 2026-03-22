use bevy::ui::widget::NodeImageMode;
use pybevy_sprite::PyTextureSlicer;
use pyo3::prelude::*;

#[pyclass(name = "NodeImageMode", frozen, eq)]
#[derive(Clone, Debug, PartialEq)]
pub enum PyNodeImageMode {
    #[pyo3(name = "Auto")]
    Auto(),
    #[pyo3(name = "Stretch")]
    Stretch(),
    #[pyo3(name = "Sliced")]
    Sliced(PyTextureSlicer),
    #[pyo3(name = "Tiled")]
    Tiled {
        tile_x: bool,
        tile_y: bool,
        stretch_value: f32,
    },
}

impl From<NodeImageMode> for PyNodeImageMode {
    fn from(mode: NodeImageMode) -> Self {
        match mode {
            NodeImageMode::Auto => PyNodeImageMode::Auto(),
            NodeImageMode::Stretch => PyNodeImageMode::Stretch(),
            NodeImageMode::Sliced(slicer) => PyNodeImageMode::Sliced(slicer.into()),
            NodeImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => PyNodeImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            },
        }
    }
}

impl From<PyNodeImageMode> for NodeImageMode {
    fn from(mode: PyNodeImageMode) -> Self {
        match mode {
            PyNodeImageMode::Auto() => NodeImageMode::Auto,
            PyNodeImageMode::Stretch() => NodeImageMode::Stretch,
            PyNodeImageMode::Sliced(slicer) => NodeImageMode::Sliced(slicer.into()),
            PyNodeImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => NodeImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            },
        }
    }
}

#[pymethods]
impl PyNodeImageMode {
    #[staticmethod]
    pub fn auto() -> Self {
        PyNodeImageMode::Auto()
    }

    #[staticmethod]
    pub fn stretch() -> Self {
        PyNodeImageMode::Stretch()
    }

    #[staticmethod]
    pub fn sliced(slicer: PyTextureSlicer) -> Self {
        PyNodeImageMode::Sliced(slicer)
    }

    #[staticmethod]
    #[pyo3(signature = (tile_x = true, tile_y = true, stretch_value = 1.0))]
    pub fn tiled(tile_x: bool, tile_y: bool, stretch_value: f32) -> Self {
        PyNodeImageMode::Tiled {
            tile_x,
            tile_y,
            stretch_value,
        }
    }

    pub fn uses_slices(&self) -> bool {
        matches!(
            self,
            PyNodeImageMode::Sliced(_) | PyNodeImageMode::Tiled { .. }
        )
    }

    fn __repr__(&self) -> String {
        match self {
            PyNodeImageMode::Auto() => "NodeImageMode.auto()".to_string(),
            PyNodeImageMode::Stretch() => "NodeImageMode.stretch()".to_string(),
            PyNodeImageMode::Sliced(_) => "NodeImageMode.sliced(...)".to_string(),
            PyNodeImageMode::Tiled {
                tile_x,
                tile_y,
                stretch_value,
            } => format!(
                "NodeImageMode.tiled(tile_x={}, tile_y={}, stretch_value={})",
                tile_x, tile_y, stretch_value
            ),
        }
    }
}
