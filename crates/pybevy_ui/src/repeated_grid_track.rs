use bevy::ui::RepeatedGridTrack;
use pyo3::prelude::*;

use crate::grid_track_repetition::extract_grid_track_repetition_from_any;

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
                value,
            ),
        })
    }

    #[staticmethod]
    pub fn percent(repetition: &Bound<'_, PyAny>, value: f32) -> PyResult<Self> {
        Ok(PyRepeatedGridTrack {
            inner: RepeatedGridTrack::percent(
                extract_grid_track_repetition_from_any(repetition)?,
                value,
            ),
        })
    }

    #[staticmethod]
    pub fn fr(repetition: u16, value: f32) -> Self {
        PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fr(repetition, value),
        }
    }

    #[staticmethod]
    pub fn flex(repetition: u16, value: f32) -> Self {
        PyRepeatedGridTrack {
            inner: RepeatedGridTrack::flex(repetition, value),
        }
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
    pub fn fit_content_px(repetition: u16, limit: f32) -> Self {
        PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fit_content_px(repetition, limit),
        }
    }

    #[staticmethod]
    pub fn fit_content_percent(repetition: u16, limit: f32) -> Self {
        PyRepeatedGridTrack {
            inner: RepeatedGridTrack::fit_content_percent(repetition, limit),
        }
    }

    pub fn __repr__(&self) -> String {
        format!("RepeatedGridTrack({:?})", self.inner)
    }
}
