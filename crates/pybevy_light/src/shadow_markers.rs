use bevy::light::{
    LightProbe, NotShadowCaster, NotShadowReceiver, TransmittedShadowReceiver, VolumetricLight,
};
use pybevy_core::PyComponent;
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(NotShadowCaster, unit, bridge)]
#[pyclass(name = "NotShadowCaster", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyNotShadowCaster;

impl From<NotShadowCaster> for PyNotShadowCaster {
    fn from(_: NotShadowCaster) -> Self {
        PyNotShadowCaster
    }
}

impl From<PyNotShadowCaster> for NotShadowCaster {
    fn from(_: PyNotShadowCaster) -> Self {
        NotShadowCaster
    }
}

impl TryFrom<&NotShadowCaster> for PyNotShadowCaster {
    type Error = PyErr;

    fn try_from(_: &NotShadowCaster) -> PyResult<Self> {
        Ok(PyNotShadowCaster)
    }
}

#[pymethods]
impl PyNotShadowCaster {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyNotShadowCaster, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "NotShadowCaster"
    }
}

#[component_storage(NotShadowReceiver, unit, bridge)]
#[pyclass(name = "NotShadowReceiver", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyNotShadowReceiver;

impl From<NotShadowReceiver> for PyNotShadowReceiver {
    fn from(_: NotShadowReceiver) -> Self {
        PyNotShadowReceiver
    }
}

impl From<PyNotShadowReceiver> for NotShadowReceiver {
    fn from(_: PyNotShadowReceiver) -> Self {
        NotShadowReceiver
    }
}

impl TryFrom<&NotShadowReceiver> for PyNotShadowReceiver {
    type Error = PyErr;

    fn try_from(_: &NotShadowReceiver) -> PyResult<Self> {
        Ok(PyNotShadowReceiver)
    }
}

#[pymethods]
impl PyNotShadowReceiver {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyNotShadowReceiver, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "NotShadowReceiver"
    }
}

#[component_storage(TransmittedShadowReceiver, unit, bridge)]
#[pyclass(name = "TransmittedShadowReceiver", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyTransmittedShadowReceiver;

impl From<TransmittedShadowReceiver> for PyTransmittedShadowReceiver {
    fn from(_: TransmittedShadowReceiver) -> Self {
        PyTransmittedShadowReceiver
    }
}

impl From<PyTransmittedShadowReceiver> for TransmittedShadowReceiver {
    fn from(_: PyTransmittedShadowReceiver) -> Self {
        TransmittedShadowReceiver
    }
}

impl TryFrom<&TransmittedShadowReceiver> for PyTransmittedShadowReceiver {
    type Error = PyErr;

    fn try_from(_: &TransmittedShadowReceiver) -> PyResult<Self> {
        Ok(PyTransmittedShadowReceiver)
    }
}

#[pymethods]
impl PyTransmittedShadowReceiver {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyTransmittedShadowReceiver, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "TransmittedShadowReceiver"
    }
}

#[component_storage(VolumetricLight, unit, bridge)]
#[pyclass(name = "VolumetricLight", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyVolumetricLight;

impl From<VolumetricLight> for PyVolumetricLight {
    fn from(_: VolumetricLight) -> Self {
        PyVolumetricLight
    }
}

impl From<PyVolumetricLight> for VolumetricLight {
    fn from(_: PyVolumetricLight) -> Self {
        VolumetricLight
    }
}

impl TryFrom<&VolumetricLight> for PyVolumetricLight {
    type Error = PyErr;

    fn try_from(_: &VolumetricLight) -> PyResult<Self> {
        Ok(PyVolumetricLight)
    }
}

#[pymethods]
impl PyVolumetricLight {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyVolumetricLight, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "VolumetricLight"
    }
}

#[component_storage(LightProbe, unit, bridge)]
#[pyclass(name = "LightProbe", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyLightProbe;

impl From<LightProbe> for PyLightProbe {
    fn from(_: LightProbe) -> Self {
        PyLightProbe
    }
}

impl From<PyLightProbe> for LightProbe {
    fn from(_: PyLightProbe) -> Self {
        LightProbe
    }
}

impl TryFrom<&LightProbe> for PyLightProbe {
    type Error = PyErr;

    fn try_from(_: &LightProbe) -> PyResult<Self> {
        Ok(PyLightProbe)
    }
}

#[pymethods]
impl PyLightProbe {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyLightProbe, PyComponent)
    }

    pub fn __repr__(&self) -> &'static str {
        "LightProbe()"
    }

    pub fn __str__(&self) -> &'static str {
        "LightProbe()"
    }
}
