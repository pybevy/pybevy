use bevy::ui::GridTrackRepetition;
use pybevy_core::{
    domain::{IntegerInput, check_for},
    public_error,
};
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::repeated_grid_track::PyRepeatedGridTrack;

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
    if let Ok(count) = obj.extract::<IntegerInput<u16>>() {
        return Ok(GridTrackRepetition::Count(checked_repetition_count(count)?));
    }
    Err(PyTypeError::new_err(
        public_error::ui_grid_track_repetition_type(obj.get_type().name()?),
    ))
}

/// A repetition count, checked against `u16` so an out-of-range int is a `ValueError`.
pub fn checked_repetition_count(count: IntegerInput<u16>) -> PyResult<u16> {
    Ok(check_for::<PyRepeatedGridTrack, _>("repetition", count, &[])?.stored())
}
