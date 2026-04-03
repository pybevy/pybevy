use bevy::render::{
    batching::NoAutomaticBatching,
    experimental::occlusion_culling::OcclusionCulling,
    view::{Hdr, NoIndirectDrawing},
};
use pybevy_core::PyComponent;
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

/// Marker component that enables HDR rendering for a camera.
///
/// When added to a camera entity, enables high dynamic range rendering.
#[pycomponent(Hdr, unit, bridge)]
#[pyclass(name = "Hdr", extends = PyComponent, frozen, eq)]
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
    pub fn new() -> (Self, PyComponent) {
        (PyHdr, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "Hdr"
    }
}

/// Marker component that disables automatic batching for an entity.
///
/// Prevents the entity from being automatically batched with similar
/// entities during rendering.
#[pycomponent(NoAutomaticBatching, unit, bridge)]
#[pyclass(name = "NoAutomaticBatching", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyNoAutomaticBatching;

impl From<NoAutomaticBatching> for PyNoAutomaticBatching {
    fn from(_: NoAutomaticBatching) -> Self {
        PyNoAutomaticBatching
    }
}

impl From<PyNoAutomaticBatching> for NoAutomaticBatching {
    fn from(_: PyNoAutomaticBatching) -> Self {
        NoAutomaticBatching
    }
}

impl TryFrom<&NoAutomaticBatching> for PyNoAutomaticBatching {
    type Error = PyErr;

    fn try_from(_: &NoAutomaticBatching) -> PyResult<Self> {
        Ok(PyNoAutomaticBatching)
    }
}

#[pymethods]
impl PyNoAutomaticBatching {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyNoAutomaticBatching, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "NoAutomaticBatching()"
    }
}

/// Marker component that disables indirect drawing for an entity.
///
/// Prevents the entity from using indirect draw calls during rendering.
#[pycomponent(NoIndirectDrawing, unit, bridge)]
#[pyclass(name = "NoIndirectDrawing", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyNoIndirectDrawing;

impl From<NoIndirectDrawing> for PyNoIndirectDrawing {
    fn from(_: NoIndirectDrawing) -> Self {
        PyNoIndirectDrawing
    }
}

impl From<PyNoIndirectDrawing> for NoIndirectDrawing {
    fn from(_: PyNoIndirectDrawing) -> Self {
        NoIndirectDrawing
    }
}

impl TryFrom<&NoIndirectDrawing> for PyNoIndirectDrawing {
    type Error = PyErr;

    fn try_from(_: &NoIndirectDrawing) -> PyResult<Self> {
        Ok(PyNoIndirectDrawing)
    }
}

#[pymethods]
impl PyNoIndirectDrawing {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyNoIndirectDrawing, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "NoIndirectDrawing()"
    }
}

/// Marker component that enables occlusion culling for a camera.
///
/// When added to a camera entity, enables GPU-based occlusion culling
/// which can improve performance by not rendering objects hidden behind
/// other objects.
#[pycomponent(OcclusionCulling, unit, bridge)]
#[pyclass(name = "OcclusionCulling", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyOcclusionCulling;

impl From<OcclusionCulling> for PyOcclusionCulling {
    fn from(_: OcclusionCulling) -> Self {
        PyOcclusionCulling
    }
}

impl From<PyOcclusionCulling> for OcclusionCulling {
    fn from(_: PyOcclusionCulling) -> Self {
        OcclusionCulling
    }
}

impl TryFrom<&OcclusionCulling> for PyOcclusionCulling {
    type Error = PyErr;

    fn try_from(_: &OcclusionCulling) -> PyResult<Self> {
        Ok(PyOcclusionCulling)
    }
}

#[pymethods]
impl PyOcclusionCulling {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyOcclusionCulling, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "OcclusionCulling()"
    }
}
