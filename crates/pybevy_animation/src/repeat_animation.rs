use bevy::animation::RepeatAnimation;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(RepeatAnimation, empty_tuple, unit_parens)]
#[pyclass(
    name = "RepeatAnimation",
    module = "pybevy.animation",
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyRepeatAnimation {
    Never(),
    #[py_bevy(tuple)]
    Count {
        value: u32,
    },
    Forever(),
}

impl Default for PyRepeatAnimation {
    fn default() -> Self {
        Self::Never()
    }
}
