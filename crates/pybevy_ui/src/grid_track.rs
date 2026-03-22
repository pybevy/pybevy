use bevy::ui::GridTrack;
use pyo3::prelude::*;

#[pyclass(name = "GridTrack", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyGridTrack {
    pub(crate) inner: GridTrack,
}

impl From<GridTrack> for PyGridTrack {
    fn from(track: GridTrack) -> Self {
        PyGridTrack { inner: track }
    }
}

impl From<PyGridTrack> for GridTrack {
    fn from(py_track: PyGridTrack) -> Self {
        py_track.inner
    }
}

impl Default for PyGridTrack {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PyGridTrack {
    #[new]
    pub fn new() -> Self {
        PyGridTrack {
            inner: GridTrack::default(),
        }
    }

    #[staticmethod]
    pub fn px(value: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::px(value),
        }
    }

    #[staticmethod]
    pub fn percent(value: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::percent(value),
        }
    }

    #[staticmethod]
    pub fn fr(value: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::fr(value),
        }
    }

    #[staticmethod]
    pub fn flex(value: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::flex(value),
        }
    }

    #[staticmethod]
    pub fn auto() -> Self {
        PyGridTrack {
            inner: GridTrack::auto(),
        }
    }

    #[staticmethod]
    pub fn min_content() -> Self {
        PyGridTrack {
            inner: GridTrack::min_content(),
        }
    }

    #[staticmethod]
    pub fn max_content() -> Self {
        PyGridTrack {
            inner: GridTrack::max_content(),
        }
    }

    #[staticmethod]
    pub fn fit_content_px(limit: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::fit_content_px(limit),
        }
    }

    #[staticmethod]
    pub fn fit_content_percent(limit: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::fit_content_percent(limit),
        }
    }

    #[staticmethod]
    pub fn vw(value: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::vw(value),
        }
    }

    #[staticmethod]
    pub fn vh(value: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::vh(value),
        }
    }

    #[staticmethod]
    pub fn vmin(value: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::vmin(value),
        }
    }

    #[staticmethod]
    pub fn vmax(value: f32) -> Self {
        PyGridTrack {
            inner: GridTrack::vmax(value),
        }
    }

    pub fn __repr__(&self) -> String {
        format!("GridTrack({:?})", self.inner)
    }
}
