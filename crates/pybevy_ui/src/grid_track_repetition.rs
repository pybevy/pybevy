use bevy::ui::GridTrackRepetition;
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyenum(GridTrackRepetition, empty_tuple)]
#[pyclass(
    name = "GridTrackRepetition",
    module = "pybevy.ui",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyGridTrackRepetition {
    #[py_bevy(tuple)]
    Count {
        value: u16,
    },
    AutoFill(),
    AutoFit(),
}

impl Default for PyGridTrackRepetition {
    fn default() -> Self {
        Self::Count { value: 1 }
    }
}

/// Accept an int (count) or a GridTrackRepetition value.
pub fn extract_grid_track_repetition_from_any(
    obj: &Bound<'_, PyAny>,
) -> PyResult<GridTrackRepetition> {
    if let Ok(repetition) = obj.extract::<PyGridTrackRepetition>() {
        return Ok(repetition.into());
    }
    if let Ok(count) = obj.extract::<u16>() {
        return Ok(GridTrackRepetition::Count(count));
    }
    Err(PyTypeError::new_err(format!(
        "Expected int or GridTrackRepetition, got {:?}",
        obj.get_type()
    )))
}
