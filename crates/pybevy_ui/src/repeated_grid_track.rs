use bevy::{
    reflect::{PartialReflect, ReflectRef},
    ui::{GridTrack, GridTrackRepetition, RepeatedGridTrack},
};
use pybevy_core::domain::{FINITE, check};
use pyo3::prelude::*;

use crate::{
    grid_track::grid_track_factory,
    grid_track_repetition::{PyGridTrackRepetition, extract_grid_track_repetition_from_any},
};

const REPEATED_FACTORIES: [&str; 9] = [
    "px",
    "percent",
    "fr",
    "flex",
    "auto",
    "min_content",
    "max_content",
    "fit_content_px",
    "fit_content_percent",
];

fn repeated_grid_track_repr(track: &RepeatedGridTrack) -> Option<String> {
    let ReflectRef::Struct(fields) = track.reflect_ref() else {
        return None;
    };
    let repetition = fields
        .field("repetition")?
        .try_downcast_ref::<GridTrackRepetition>()?;
    let ReflectRef::List(tracks) = fields.field("tracks")?.reflect_ref() else {
        return None;
    };
    if tracks.len() != 1 {
        return None;
    }
    let single = tracks.get(0)?.try_downcast_ref::<GridTrack>()?;
    let (name, argument) = grid_track_factory(single)?;
    if !REPEATED_FACTORIES.contains(&name) {
        return None;
    }
    let repetition = match repetition {
        GridTrackRepetition::Count(count) => count.to_string(),
        other => PyGridTrackRepetition::from(*other).__repr__(),
    };
    let arguments = match argument {
        Some(argument) => format!("{repetition}, {argument}"),
        None => repetition,
    };
    Some(format!("RepeatedGridTrack.{name}({arguments})"))
}

#[pyclass(
    name = "RepeatedGridTrack",
    module = "pybevy.ui",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyRepeatedGridTrack {
    pub(crate) inner: RepeatedGridTrack,
}

impl From<RepeatedGridTrack> for PyRepeatedGridTrack {
    fn from(track: RepeatedGridTrack) -> Self {
        PyRepeatedGridTrack { inner: track }
    }
}

impl From<PyRepeatedGridTrack> for RepeatedGridTrack {
    fn from(py_track: PyRepeatedGridTrack) -> Self {
        py_track.inner
    }
}

impl Default for PyRepeatedGridTrack {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PyRepeatedGridTrack {
    #[new]
    pub fn new() -> Self {
        PyRepeatedGridTrack {
            inner: RepeatedGridTrack::default(),
        }
    }

    #[staticmethod]
    pub fn px(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::px(
                extract_grid_track_repetition_from_any(repetition)?,
                check("RepeatedGridTrack", "px", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn percent(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::percent(
                extract_grid_track_repetition_from_any(repetition)?,
                check("RepeatedGridTrack", "percent", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn fr(repetition: u16, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fr(
                repetition,
                check("RepeatedGridTrack", "fr", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn flex(repetition: u16, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::flex(
                repetition,
                check("RepeatedGridTrack", "flex", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn auto(repetition: u16) -> Self {
        PyRepeatedGridTrack {
            inner: RepeatedGridTrack::auto(repetition),
        }
    }

    #[staticmethod]
    pub fn min_content(repetition: u16) -> Self {
        PyRepeatedGridTrack {
            inner: RepeatedGridTrack::min_content(repetition),
        }
    }

    #[staticmethod]
    pub fn max_content(repetition: u16) -> Self {
        PyRepeatedGridTrack {
            inner: RepeatedGridTrack::max_content(repetition),
        }
    }

    #[staticmethod]
    pub fn fit_content_px(repetition: u16, limit: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fit_content_px(
                repetition,
                check("RepeatedGridTrack", "fit_content_px", limit, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn fit_content_percent(repetition: u16, limit: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fit_content_percent(
                repetition,
                check("RepeatedGridTrack", "fit_content_percent", limit, FINITE)?,
            ),
        })
    }

    pub fn __repr__(&self) -> String {
        repeated_grid_track_repr(&self.inner).unwrap_or_else(|| "RepeatedGridTrack(...)".into())
    }
}
