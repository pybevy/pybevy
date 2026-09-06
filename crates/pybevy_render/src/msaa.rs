use bevy::render::view::Msaa;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pywrap(Msaa, bridge, copy)]
#[pyclass(name = "Msaa", module = "pybevy.render", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
        let msaa = match samples {
            1 => Msaa::Off,
            2 => Msaa::Sample2,
            4 => Msaa::Sample4,
            8 => Msaa::Sample8,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unsupported MSAA sample count: {samples}"
                )));
            }
        };
        Py::new(py, Self::from_owned(msaa))
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
