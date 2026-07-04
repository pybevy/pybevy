use bevy::{math::UVec2, window::VideoMode};
use pybevy_math::uvec2::PyUVec2;
use pyo3::prelude::*;

#[pyclass(name = "VideoMode", eq, frozen, from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyVideoMode {
    pub physical_size: UVec2,
    pub bit_depth: u16,
    pub refresh_rate_millihertz: u32,
}

impl From<VideoMode> for PyVideoMode {
    fn from(vm: VideoMode) -> Self {
        PyVideoMode {
            physical_size: vm.physical_size,
            bit_depth: vm.bit_depth,
            refresh_rate_millihertz: vm.refresh_rate_millihertz,
        }
    }
}

impl From<PyVideoMode> for VideoMode {
    fn from(vm: PyVideoMode) -> Self {
        VideoMode {
            physical_size: vm.physical_size,
            bit_depth: vm.bit_depth,
            refresh_rate_millihertz: vm.refresh_rate_millihertz,
        }
    }
}

#[pymethods]
impl PyVideoMode {
    #[new]
    pub fn new(physical_size: PyUVec2, bit_depth: u16, refresh_rate_millihertz: u32) -> Self {
        PyVideoMode {
            physical_size: physical_size.into(),
            bit_depth,
            refresh_rate_millihertz,
        }
    }

    #[getter]
    pub fn physical_size(&self) -> PyUVec2 {
        self.physical_size.into()
    }

    #[getter]
    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    #[getter]
    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    pub fn __repr__(&self) -> String {
        format!(
            "VideoMode({}x{}, {}bpp, {}mHz)",
            self.physical_size.x,
            self.physical_size.y,
            self.bit_depth,
            self.refresh_rate_millihertz
        )
    }
}
