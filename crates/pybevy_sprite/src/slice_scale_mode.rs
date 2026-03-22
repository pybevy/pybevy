use bevy::sprite::SliceScaleMode;
use pyo3::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PySliceScaleModeInner {
    Stretch,
    Tile { stretch_value: f32 },
}

impl From<PySliceScaleModeInner> for SliceScaleMode {
    fn from(mode: PySliceScaleModeInner) -> Self {
        match mode {
            PySliceScaleModeInner::Stretch => SliceScaleMode::Stretch,
            PySliceScaleModeInner::Tile { stretch_value } => SliceScaleMode::Tile { stretch_value },
        }
    }
}

impl From<SliceScaleMode> for PySliceScaleModeInner {
    fn from(mode: SliceScaleMode) -> Self {
        match mode {
            SliceScaleMode::Stretch => PySliceScaleModeInner::Stretch,
            SliceScaleMode::Tile { stretch_value } => PySliceScaleModeInner::Tile { stretch_value },
        }
    }
}

#[pyclass(name = "SliceScaleMode", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PySliceScaleMode {
    pub(crate) inner: PySliceScaleModeInner,
}

#[pymethods]
impl PySliceScaleMode {
    #[classattr]
    const STRETCH: Self = PySliceScaleMode {
        inner: PySliceScaleModeInner::Stretch,
    };

    #[new]
    pub fn new() -> Self {
        PySliceScaleMode {
            inner: PySliceScaleModeInner::Stretch,
        }
    }

    #[staticmethod]
    #[pyo3(signature = (stretch_value = 1.0))]
    pub fn tile(stretch_value: f32) -> Self {
        PySliceScaleMode {
            inner: PySliceScaleModeInner::Tile { stretch_value },
        }
    }

    pub fn __repr__(&self) -> String {
        match &self.inner {
            PySliceScaleModeInner::Stretch => "SliceScaleMode.STRETCH".to_string(),
            PySliceScaleModeInner::Tile { stretch_value } => {
                format!("SliceScaleMode.tile({})", stretch_value)
            }
        }
    }
}

impl From<PySliceScaleMode> for SliceScaleMode {
    fn from(wrapper: PySliceScaleMode) -> Self {
        wrapper.inner.into()
    }
}

impl From<SliceScaleMode> for PySliceScaleMode {
    fn from(mode: SliceScaleMode) -> Self {
        PySliceScaleMode { inner: mode.into() }
    }
}

impl Default for PySliceScaleMode {
    fn default() -> Self {
        Self::new()
    }
}
