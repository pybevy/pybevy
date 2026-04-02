use bevy::{camera::MainPassResolutionOverride, math::UVec2};
use pybevy_core::PyComponent;
use pybevy_math::PyUVec2;
use pyo3::prelude::*;

#[pyclass(name = "MainPassResolutionOverride", extends = PyComponent, frozen, eq)]
#[derive(Debug, PartialEq)]
pub struct PyMainPassResolutionOverride {
    resolution: UVec2,
}

impl PyMainPassResolutionOverride {
    pub fn to_bevy(&self) -> MainPassResolutionOverride {
        MainPassResolutionOverride(self.resolution)
    }
}

impl From<MainPassResolutionOverride> for PyMainPassResolutionOverride {
    fn from(value: MainPassResolutionOverride) -> Self {
        PyMainPassResolutionOverride {
            resolution: value.0,
        }
    }
}

#[pymethods]
impl PyMainPassResolutionOverride {
    #[new]
    pub fn new(resolution: &PyUVec2) -> (Self, PyComponent) {
        let res: UVec2 = resolution.into();
        (
            PyMainPassResolutionOverride { resolution: res },
            PyComponent,
        )
    }

    #[getter]
    pub fn resolution(&self) -> PyUVec2 {
        self.resolution.into()
    }

    #[getter]
    pub fn width(&self) -> u32 {
        self.resolution.x
    }

    #[getter]
    pub fn height(&self) -> u32 {
        self.resolution.y
    }

    pub fn __repr__(&self) -> String {
        format!(
            "MainPassResolutionOverride({}x{})",
            self.resolution.x, self.resolution.y
        )
    }
}
