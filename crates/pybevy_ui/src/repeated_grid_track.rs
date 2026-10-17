use bevy::{
    reflect::{PartialReflect, ReflectRef},
    ui::{GridTrack, GridTrackRepetition, RepeatedGridTrack},
};
use pybevy_core::{
    FieldStorage,
    domain::{FINITE, IntegerInput, check_for},
    public_error::{REPEATED_GRID_TRACK_EMPTY, grid_template_track_type},
};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyList,
};

use crate::{
    grid_track::{PyGridTrack, grid_track_factory},
    grid_track_repetition::{
        PyGridTrackRepetition, checked_repetition_count, extract_grid_track_repetition_from_any,
    },
    track_sizing_function::{
        PyMaxTrackSizingFunction, PyMinTrackSizingFunction, checked_max, checked_min,
    },
};

const REPEATED_FACTORIES: [&str; 15] = [
    "px",
    "percent",
    "em",
    "rem",
    "fr",
    "flex",
    "auto",
    "min_content",
    "max_content",
    "fit_content_px",
    "fit_content_percent",
    "vmin",
    "vmax",
    "vh",
    "vw",
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

/// Mirrors bevy's `impl From<GridTrack> for RepeatedGridTrack`: a bare track is
/// one repetition of itself.
pub fn extract_repeated_grid_track_from_any(
    value: &Bound<'_, PyAny>,
) -> PyResult<RepeatedGridTrack> {
    if let Ok(track) = value.cast::<PyRepeatedGridTrack>() {
        return Ok(track.get().inner.clone());
    }
    if let Ok(track) = value.cast::<PyGridTrack>() {
        return Ok(RepeatedGridTrack::from(track.get().inner));
    }
    let type_name = value.get_type().name()?;
    Err(PyTypeError::new_err(grid_template_track_type(
        &type_name.to_string_lossy(),
    )))
}

#[pyclass(
    name = "_RepeatedGridTracks",
    module = "pybevy.ui",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRepeatedGridTracks {
    storage: FieldStorage<Vec<RepeatedGridTrack>>,
}

pybevy_core::impl_live_value_list!(
    PyRepeatedGridTracks,
    "_RepeatedGridTracks",
    Vec<RepeatedGridTrack>,
    RepeatedGridTrack,
    PyRepeatedGridTrack,
    extract = extract_repeated_grid_track_from_any
);

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
                check_for::<Self, _>("px", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn percent(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::percent(
                extract_grid_track_repetition_from_any(repetition)?,
                check_for::<Self, _>("percent", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn em(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::em(
                extract_grid_track_repetition_from_any(repetition)?,
                check_for::<Self, _>("em", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn rem(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::rem(
                extract_grid_track_repetition_from_any(repetition)?,
                check_for::<Self, _>("rem", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn fr(repetition: IntegerInput<u16>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fr(
                checked_repetition_count(repetition)?,
                check_for::<Self, _>("fr", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn flex(repetition: IntegerInput<u16>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::flex(
                checked_repetition_count(repetition)?,
                check_for::<Self, _>("flex", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn auto(repetition: IntegerInput<u16>) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::auto(checked_repetition_count(repetition)?),
        })
    }

    #[staticmethod]
    pub fn min_content(repetition: IntegerInput<u16>) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::min_content(checked_repetition_count(repetition)?),
        })
    }

    #[staticmethod]
    pub fn max_content(repetition: IntegerInput<u16>) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::max_content(checked_repetition_count(repetition)?),
        })
    }

    #[staticmethod]
    pub fn fit_content_px(repetition: IntegerInput<u16>, limit: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fit_content_px(
                checked_repetition_count(repetition)?,
                check_for::<Self, _>("fit_content_px", limit, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn fit_content_percent(repetition: IntegerInput<u16>, limit: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fit_content_percent(
                checked_repetition_count(repetition)?,
                check_for::<Self, _>("fit_content_percent", limit, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn minmax(
        repetition: &Bound<'_, PyAny>,
        min: PyMinTrackSizingFunction,
        max: PyMaxTrackSizingFunction,
    ) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::minmax(
                extract_grid_track_repetition_from_any(repetition)?,
                checked_min(min)?,
                checked_max(max)?,
            ),
        })
    }

    #[staticmethod]
    pub fn vmin(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::vmin(
                extract_grid_track_repetition_from_any(repetition)?,
                check_for::<Self, _>("vmin", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn vmax(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::vmax(
                extract_grid_track_repetition_from_any(repetition)?,
                check_for::<Self, _>("vmax", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn vh(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::vh(
                extract_grid_track_repetition_from_any(repetition)?,
                check_for::<Self, _>("vh", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn vw(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::vw(
                extract_grid_track_repetition_from_any(repetition)?,
                check_for::<Self, _>("vw", value, FINITE)?,
            ),
        })
    }

    #[staticmethod]
    pub fn repeat_many(repetition: &Bound<'_, PyAny>, tracks: Vec<PyGridTrack>) -> PyResult<Self> {
        if tracks.is_empty() {
            return Err(PyValueError::new_err(REPEATED_GRID_TRACK_EMPTY));
        }
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::repeat_many(
                extract_grid_track_repetition_from_any(repetition)?,
                tracks.into_iter().map(GridTrack::from).collect::<Vec<_>>(),
            ),
        })
    }

    pub fn __copy__(&self) -> Self {
        self.clone()
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    pub fn __repr__(&self) -> String {
        repeated_grid_track_repr(&self.inner).unwrap_or_else(|| "RepeatedGridTrack(...)".into())
    }
}
