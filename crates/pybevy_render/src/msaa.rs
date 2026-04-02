use bevy::render::view::Msaa;
use pybevy_core::PyComponent;
use pybevy_macros::newtype_storage;
use pyo3::prelude::*;

#[newtype_storage(Msaa, bridge, copy)]
#[pyclass(name = "Msaa", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyMsaa(pub(crate) Msaa);

#[pymethods]
impl PyMsaa {
    #[new]
    pub fn new(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Msaa::default()))
    }

    #[classattr]
    #[pyo3(name = "Off")]
    pub fn off(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Msaa::Off))
    }

    #[classattr]
    #[pyo3(name = "Sample2")]
    pub fn sample2(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Msaa::Sample2))
    }

    #[classattr]
    #[pyo3(name = "Sample4")]
    pub fn sample4(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Msaa::Sample4))
    }

    #[classattr]
    #[pyo3(name = "Sample8")]
    pub fn sample8(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Msaa::Sample8))
    }

    pub fn samples(&self) -> u32 {
        self.0.samples()
    }

    #[staticmethod]
    pub fn from_samples(py: Python, samples: u32) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Msaa::from_samples(samples)))
    }

    pub fn __repr__(&self) -> &'static str {
        match self.0 {
            Msaa::Off => "Msaa.Off",
            Msaa::Sample2 => "Msaa.Sample2",
            Msaa::Sample4 => "Msaa.Sample4",
            Msaa::Sample8 => "Msaa.Sample8",
        }
    }
}
