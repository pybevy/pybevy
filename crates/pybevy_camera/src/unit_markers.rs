use bevy::camera::{
    Camera2d, Hdr, ShadowLodOrigin,
    visibility::{NoCpuCulling, NoFrustumCulling},
};
use pybevy_core::PyComponent;
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(NoCpuCulling, unit, bridge, no_reflect)]
#[pyclass(name = "NoCpuCulling", module = "pybevy.camera", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyNoCpuCulling, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "NoCpuCulling()"
    }
}

#[pycomponent(NoFrustumCulling, unit, bridge)]
#[pyclass(name = "NoFrustumCulling", module = "pybevy.camera", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyNoFrustumCulling, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "NoFrustumCulling()"
    }
}

#[pycomponent(Camera2d, unit, bridge)]
#[pyclass(name = "Camera2d", module = "pybevy.camera", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyCamera2d, PyComponent).into()
    }

    pub fn __copy__(&self, py: Python) -> PyResult<Py<Self>> {
        Py::new(py, (PyCamera2d, PyComponent))
    }

    pub fn __deepcopy__(&self, py: Python, _memo: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        self.__copy__(py)
    }

    pub fn __repr__(&self) -> &'static str {
        "Camera2d"
    }
}

#[pycomponent(ShadowLodOrigin, unit, bridge)]
#[pyclass(
    name = "ShadowLodOrigin", module = "pybevy.camera",
    extends = PyComponent,
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyShadowLodOrigin;

impl From<ShadowLodOrigin> for PyShadowLodOrigin {
    fn from(_: ShadowLodOrigin) -> Self {
        PyShadowLodOrigin
    }
}

impl From<PyShadowLodOrigin> for ShadowLodOrigin {
    fn from(_: PyShadowLodOrigin) -> Self {
        ShadowLodOrigin
    }
}

impl TryFrom<&ShadowLodOrigin> for PyShadowLodOrigin {
    type Error = PyErr;

    fn try_from(_: &ShadowLodOrigin) -> PyResult<Self> {
        Ok(PyShadowLodOrigin)
    }
}

#[pymethods]
impl PyShadowLodOrigin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyShadowLodOrigin, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "ShadowLodOrigin()"
    }
}

/// Marker component that enables HDR rendering for a camera.
///
/// When added to a camera entity, enables high dynamic range rendering.
#[pycomponent(Hdr, unit, bridge)]
#[pyclass(name = "Hdr", module = "pybevy.camera", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyHdr;

impl From<Hdr> for PyHdr {
    fn from(_: Hdr) -> Self {
        PyHdr
    }
}

impl From<PyHdr> for Hdr {
    fn from(_: PyHdr) -> Self {
        Hdr
    }
}

impl TryFrom<&Hdr> for PyHdr {
    type Error = PyErr;

    fn try_from(_: &Hdr) -> PyResult<Self> {
        Ok(PyHdr)
    }
}

#[pymethods]
impl PyHdr {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyHdr, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "Hdr"
    }
}
