use bevy::window::VideoMode;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::uvec2::PyUVec2;
use pyo3::prelude::*;

#[pyvalue]
#[pyclass(
    name = "VideoMode",
    module = "pybevy.window",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyVideoMode {
    pub(crate) storage: ValueStorage<VideoMode>,
}

impl PartialEq for PyVideoMode {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.to_bevy(), other.to_bevy()), (Ok(a), Ok(b)) if a == b)
    }
}

impl Eq for PyVideoMode {}

impl From<VideoMode> for PyVideoMode {
    fn from(value: VideoMode) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyVideoMode> for VideoMode {
    type Error = PyErr;

    fn try_from(value: PyVideoMode) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyVideoMode> for VideoMode {
    type Error = PyErr;

    fn try_from(value: &PyVideoMode) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyVideoMode {
    #[new]
    #[pyo3(signature = (*, physical_size, bit_depth, refresh_rate_millihertz))]
    pub fn new(
        physical_size: PyUVec2,
        bit_depth: u16,
        refresh_rate_millihertz: u32,
    ) -> PyResult<Self> {
        Ok(Self::from_owned(VideoMode {
            physical_size: physical_size.try_into()?,
            bit_depth,
            refresh_rate_millihertz,
        }))
    }

    #[getter]
    pub fn physical_size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_field_as(|mode| &mode.physical_size)?)
    }

    #[getter]
    pub fn bit_depth(&self) -> PyResult<u16> {
        Ok(self.as_ref()?.bit_depth)
    }

    #[getter]
    pub fn refresh_rate_millihertz(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.refresh_rate_millihertz)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let mode = self.to_bevy()?;
        Ok(format!(
            "VideoMode({}x{}, {}bpp, {}mHz)",
            mode.physical_size.x,
            mode.physical_size.y,
            mode.bit_depth,
            mode.refresh_rate_millihertz
        ))
    }
}
