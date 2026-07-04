use pyo3::prelude::*;

#[pyclass(name = "Fixed", frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PyFixed;

#[pymethods]
impl PyFixed {
    #[new]
    pub fn new() -> Self {
        PyFixed
    }

    pub fn __repr__(&self) -> &'static str {
        "Fixed"
    }
}

#[pyclass(name = "Real", frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PyReal;

#[pymethods]
impl PyReal {
    #[new]
    pub fn new() -> Self {
        PyReal
    }

    pub fn __repr__(&self) -> &'static str {
        "Real"
    }
}

#[pyclass(name = "Virtual", frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PyVirtual;

#[pymethods]
impl PyVirtual {
    #[new]
    pub fn new() -> Self {
        PyVirtual
    }

    pub fn __repr__(&self) -> &'static str {
        "Virtual"
    }
}
