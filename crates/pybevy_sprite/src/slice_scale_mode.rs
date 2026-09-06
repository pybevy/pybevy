use bevy::sprite::SliceScaleMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(SliceScaleMode, empty_tuple, unit_parens)]
#[pyclass(
    name = "SliceScaleMode",
    module = "pybevy.sprite",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PySliceScaleMode {
    Stretch(),
    #[pyo3(constructor = (stretch_value = 1.0))]
    Tile {
        stretch_value: f32,
    },
}

impl Default for PySliceScaleMode {
    fn default() -> Self {
        SliceScaleMode::default().into()
    }
}
