use bevy::ui::GridTrack;
use pyo3::{exceptions::PyValueError, prelude::*};

pub(crate) fn finite_grid_value(value: f32, parameter: &str) -> PyResult<f32> {
    if !value.is_finite() {
        return Err(PyValueError::new_err(format!(
            "grid track {parameter} must be finite (got {value})"
        )));
    }
    Ok(value)
}

#[pyclass(name = "GridTrack", module = "pybevy.ui", eq, frozen, from_py_object)]
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
    pub fn px(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::px(finite_grid_value(value, "px")?),
        })
    }

    #[staticmethod]
    pub fn percent(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::percent(finite_grid_value(value, "percent")?),
        })
    }

    #[staticmethod]
    pub fn fr(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::fr(finite_grid_value(value, "fr")?),
        })
    }

    #[staticmethod]
    pub fn flex(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::flex(finite_grid_value(value, "flex")?),
        })
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
    pub fn fit_content_px(limit: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::fit_content_px(finite_grid_value(limit, "fit_content_px")?),
        })
    }

    #[staticmethod]
    pub fn fit_content_percent(limit: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::fit_content_percent(finite_grid_value(limit, "fit_content_percent")?),
        })
    }

    #[staticmethod]
    pub fn vw(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::vw(finite_grid_value(value, "vw")?),
        })
    }

    #[staticmethod]
    pub fn vh(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::vh(finite_grid_value(value, "vh")?),
        })
    }

    #[staticmethod]
    pub fn vmin(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::vmin(finite_grid_value(value, "vmin")?),
        })
    }

    #[staticmethod]
    pub fn vmax(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::vmax(finite_grid_value(value, "vmax")?),
        })
    }

    pub fn __repr__(&self) -> String {
        format!("GridTrack({:?})", self.inner)
    }
}
