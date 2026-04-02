use bevy::{
    camera::{
        Camera2d,
        visibility::{NoCpuCulling, NoFrustumCulling},
    },
    core_pipeline::prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass},
};
use pybevy_core::PyComponent;
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(NoCpuCulling, unit, bridge)]
#[pyclass(name = "NoCpuCulling", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyNoCpuCulling;

impl From<NoCpuCulling> for PyNoCpuCulling {
    fn from(_: NoCpuCulling) -> Self {
        PyNoCpuCulling
    }
}

impl From<PyNoCpuCulling> for NoCpuCulling {
    fn from(_: PyNoCpuCulling) -> Self {
        NoCpuCulling
    }
}

impl TryFrom<&NoCpuCulling> for PyNoCpuCulling {
    type Error = PyErr;

    fn try_from(_: &NoCpuCulling) -> PyResult<Self> {
        Ok(PyNoCpuCulling)
    }
}

#[pymethods]
impl PyNoCpuCulling {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyNoCpuCulling, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "NoCpuCulling()"
    }
}

#[component_storage(NoFrustumCulling, unit, bridge)]
#[pyclass(name = "NoFrustumCulling", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyNoFrustumCulling;

impl From<NoFrustumCulling> for PyNoFrustumCulling {
    fn from(_: NoFrustumCulling) -> Self {
        PyNoFrustumCulling
    }
}

impl From<PyNoFrustumCulling> for NoFrustumCulling {
    fn from(_: PyNoFrustumCulling) -> Self {
        NoFrustumCulling
    }
}

impl TryFrom<&NoFrustumCulling> for PyNoFrustumCulling {
    type Error = PyErr;

    fn try_from(_: &NoFrustumCulling) -> PyResult<Self> {
        Ok(PyNoFrustumCulling)
    }
}

#[pymethods]
impl PyNoFrustumCulling {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyNoFrustumCulling, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "NoFrustumCulling()"
    }
}

#[component_storage(Camera2d, unit, bridge)]
#[pyclass(name = "Camera2d", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyCamera2d;

impl From<Camera2d> for PyCamera2d {
    fn from(_: Camera2d) -> Self {
        PyCamera2d
    }
}

impl From<PyCamera2d> for Camera2d {
    fn from(_: PyCamera2d) -> Self {
        Camera2d
    }
}

impl TryFrom<&Camera2d> for PyCamera2d {
    type Error = PyErr;

    fn try_from(_: &Camera2d) -> PyResult<Self> {
        Ok(PyCamera2d)
    }
}

#[pymethods]
impl PyCamera2d {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyCamera2d, PyComponent)
    }

    pub fn __copy__(&self, py: Python) -> PyResult<Py<Self>> {
        Py::new(py, (PyCamera2d, PyComponent))
    }

    pub fn __repr__(&self) -> &'static str {
        "Camera2d"
    }
}

#[component_storage(DepthPrepass, unit, bridge)]
#[pyclass(name = "DepthPrepass", extends = PyComponent, frozen, eq)]
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
    pub fn new() -> (Self, PyComponent) {
        (PyDepthPrepass, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "DepthPrepass()"
    }
}

#[component_storage(NormalPrepass, unit, bridge)]
#[pyclass(name = "NormalPrepass", extends = PyComponent, frozen, eq)]
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
    pub fn new() -> (Self, PyComponent) {
        (PyNormalPrepass, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "NormalPrepass()"
    }
}

#[component_storage(MotionVectorPrepass, unit, bridge)]
#[pyclass(name = "MotionVectorPrepass", extends = PyComponent, frozen, eq)]
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
    pub fn new() -> (Self, PyComponent) {
        (PyMotionVectorPrepass, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "MotionVectorPrepass()"
    }
}

#[component_storage(DeferredPrepass, unit, bridge)]
#[pyclass(name = "DeferredPrepass", extends = PyComponent, frozen, eq)]
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
    pub fn new() -> (Self, PyComponent) {
        (PyDeferredPrepass, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "DeferredPrepass()"
    }
}
