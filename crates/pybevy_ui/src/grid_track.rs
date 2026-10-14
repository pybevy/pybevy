use bevy::{
    reflect::{PartialReflect, ReflectRef},
    ui::{GridTrack, MaxTrackSizingFunction, MinTrackSizingFunction},
};
use pybevy_core::{
    domain::{FINITE, check},
    repr::float_repr,
};
use pyo3::prelude::*;

/// The factory name and arguments that rebuild `track`, when one exists.
pub(crate) fn grid_track_factory(track: &GridTrack) -> Option<(&'static str, Option<String>)> {
    let ReflectRef::Struct(fields) = track.reflect_ref() else {
        return None;
    };
    let min = fields
        .field("min_sizing_function")?
        .try_downcast_ref::<MinTrackSizingFunction>()?;
    let max = fields
        .field("max_sizing_function")?
        .try_downcast_ref::<MaxTrackSizingFunction>()?;
    let value = |value: &f32| Some(float_repr(*value));
    Some(match (min, max) {
        (MinTrackSizingFunction::Px(a), MaxTrackSizingFunction::Px(b)) if a == b => {
            ("px", value(a))
        }
        (MinTrackSizingFunction::Percent(a), MaxTrackSizingFunction::Percent(b)) if a == b => {
            ("percent", value(a))
        }
        (MinTrackSizingFunction::Auto, MaxTrackSizingFunction::Fraction(v)) => ("fr", value(v)),
        (MinTrackSizingFunction::Px(zero), MaxTrackSizingFunction::Fraction(v)) if *zero == 0.0 => {
            ("flex", value(v))
        }
        (MinTrackSizingFunction::Auto, MaxTrackSizingFunction::Auto) => ("auto", None),
        (MinTrackSizingFunction::MinContent, MaxTrackSizingFunction::MinContent) => {
            ("min_content", None)
        }
        (MinTrackSizingFunction::MaxContent, MaxTrackSizingFunction::MaxContent) => {
            ("max_content", None)
        }
        (MinTrackSizingFunction::Auto, MaxTrackSizingFunction::FitContentPx(v)) => {
            ("fit_content_px", value(v))
        }
        (MinTrackSizingFunction::Auto, MaxTrackSizingFunction::FitContentPercent(v)) => {
            ("fit_content_percent", value(v))
        }
        (MinTrackSizingFunction::VMin(a), MaxTrackSizingFunction::VMin(b)) if a == b => {
            ("vmin", value(a))
        }
        (MinTrackSizingFunction::VMax(a), MaxTrackSizingFunction::VMax(b)) if a == b => {
            ("vmax", value(a))
        }
        (MinTrackSizingFunction::Vh(a), MaxTrackSizingFunction::Vh(b)) if a == b => {
            ("vh", value(a))
        }
        (MinTrackSizingFunction::Vw(a), MaxTrackSizingFunction::Vw(b)) if a == b => {
            ("vw", value(a))
        }
        _ => return None,
    })
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
            inner: GridTrack::px(check("GridTrack", "px", value, FINITE)?),
        })
    }

    #[staticmethod]
    pub fn percent(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::percent(check("GridTrack", "percent", value, FINITE)?),
        })
    }

    #[staticmethod]
    pub fn fr(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::fr(check("GridTrack", "fr", value, FINITE)?),
        })
    }

    #[staticmethod]
    pub fn flex(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::flex(check("GridTrack", "flex", value, FINITE)?),
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
            inner: GridTrack::fit_content_px(check("GridTrack", "fit_content_px", limit, FINITE)?),
        })
    }

    #[staticmethod]
    pub fn fit_content_percent(limit: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::fit_content_percent(check(
                "GridTrack",
                "fit_content_percent",
                limit,
                FINITE,
            )?),
        })
    }

    #[staticmethod]
    pub fn vw(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::vw(check("GridTrack", "vw", value, FINITE)?),
        })
    }

    #[staticmethod]
    pub fn vh(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::vh(check("GridTrack", "vh", value, FINITE)?),
        })
    }

    #[staticmethod]
    pub fn vmin(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::vmin(check("GridTrack", "vmin", value, FINITE)?),
        })
    }

    #[staticmethod]
    pub fn vmax(value: f32) -> PyResult<Self> {
        Ok(PyGridTrack {
            inner: GridTrack::vmax(check("GridTrack", "vmax", value, FINITE)?),
        })
    }

    pub fn __repr__(&self) -> String {
        match grid_track_factory(&self.inner) {
            Some((name, argument)) => {
                format!("GridTrack.{name}({})", argument.unwrap_or_default())
            }
            None => "GridTrack(...)".to_string(),
        }
    }
}
