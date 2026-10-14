use bevy::core_pipeline::prepass::{
    DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass,
};
use pybevy_core::PyComponent;
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(DepthPrepass, unit, bridge)]
#[pyclass(name = "DepthPrepass", module = "pybevy.core_pipeline", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyDepthPrepass;

impl From<DepthPrepass> for PyDepthPrepass {
    fn from(_: DepthPrepass) -> Self {
        PyDepthPrepass
    }
}

impl From<PyDepthPrepass> for DepthPrepass {
    fn from(_: PyDepthPrepass) -> Self {
        DepthPrepass
    }
}

impl TryFrom<&DepthPrepass> for PyDepthPrepass {
    type Error = PyErr;

    fn try_from(_: &DepthPrepass) -> PyResult<Self> {
        Ok(PyDepthPrepass)
    }
}

#[pymethods]
impl PyDepthPrepass {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyDepthPrepass, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "DepthPrepass()"
    }
}

#[pycomponent(NormalPrepass, unit, bridge)]
#[pyclass(name = "NormalPrepass", module = "pybevy.core_pipeline", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyNormalPrepass;

impl From<NormalPrepass> for PyNormalPrepass {
    fn from(_: NormalPrepass) -> Self {
        PyNormalPrepass
    }
}

impl From<PyNormalPrepass> for NormalPrepass {
    fn from(_: PyNormalPrepass) -> Self {
        NormalPrepass
    }
}

impl TryFrom<&NormalPrepass> for PyNormalPrepass {
    type Error = PyErr;

    fn try_from(_: &NormalPrepass) -> PyResult<Self> {
        Ok(PyNormalPrepass)
    }
}

#[pymethods]
impl PyNormalPrepass {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyNormalPrepass, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "NormalPrepass()"
    }
}

#[pycomponent(MotionVectorPrepass, unit, bridge)]
#[pyclass(name = "MotionVectorPrepass", module = "pybevy.core_pipeline", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyMotionVectorPrepass;

impl From<MotionVectorPrepass> for PyMotionVectorPrepass {
    fn from(_: MotionVectorPrepass) -> Self {
        PyMotionVectorPrepass
    }
}

impl From<PyMotionVectorPrepass> for MotionVectorPrepass {
    fn from(_: PyMotionVectorPrepass) -> Self {
        MotionVectorPrepass
    }
}

impl TryFrom<&MotionVectorPrepass> for PyMotionVectorPrepass {
    type Error = PyErr;

    fn try_from(_: &MotionVectorPrepass) -> PyResult<Self> {
        Ok(PyMotionVectorPrepass)
    }
}

#[pymethods]
impl PyMotionVectorPrepass {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyMotionVectorPrepass, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "MotionVectorPrepass()"
    }
}

#[pycomponent(DeferredPrepass, unit, bridge)]
#[pyclass(name = "DeferredPrepass", module = "pybevy.core_pipeline", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyDeferredPrepass;

impl From<DeferredPrepass> for PyDeferredPrepass {
    fn from(_: DeferredPrepass) -> Self {
        PyDeferredPrepass
    }
}

impl From<PyDeferredPrepass> for DeferredPrepass {
    fn from(_: PyDeferredPrepass) -> Self {
        DeferredPrepass
    }
}

impl TryFrom<&DeferredPrepass> for PyDeferredPrepass {
    type Error = PyErr;

    fn try_from(_: &DeferredPrepass) -> PyResult<Self> {
        Ok(PyDeferredPrepass)
    }
}

#[pymethods]
impl PyDeferredPrepass {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyDeferredPrepass, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "DeferredPrepass()"
    }
}
