use bevy::sprite::SliceScaleMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(SliceScaleMode, manual)]
#[pyclass(name = "SliceScaleMode", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PySliceScaleMode {
    Stretch(),
    #[pyo3(constructor = (stretch_value = 1.0))]
    Tile {
        stretch_value: f32,
    },
}

impl From<PySliceScaleMode> for SliceScaleMode {
    fn from(mode: PySliceScaleMode) -> Self {
        match mode {
            PySliceScaleMode::Stretch() => SliceScaleMode::Stretch,
            PySliceScaleMode::Tile { stretch_value } => SliceScaleMode::Tile { stretch_value },
        }
    }
}

impl From<SliceScaleMode> for PySliceScaleMode {
    fn from(mode: SliceScaleMode) -> Self {
        match mode {
            SliceScaleMode::Stretch => PySliceScaleMode::Stretch(),
            SliceScaleMode::Tile { stretch_value } => PySliceScaleMode::Tile { stretch_value },
        }
    }
}

#[pymethods]
impl PySliceScaleMode {
    pub fn __repr__(&self) -> String {
        match self {
            Self::Stretch() => "SliceScaleMode.Stretch()".to_string(),
            Self::Tile { stretch_value } => {
                format!("SliceScaleMode.Tile(stretch_value={stretch_value})")
            }
        }
    }
}

impl Default for PySliceScaleMode {
    fn default() -> Self {
        SliceScaleMode::default().into()
    }
}
