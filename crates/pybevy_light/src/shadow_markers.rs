use bevy::light::{
    LightProbe, NotShadowCaster, NotShadowReceiver, TransmittedShadowReceiver, VolumetricLight,
};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pycomponent(NotShadowCaster, unit, bridge)]
#[pyclass(name = "NotShadowCaster", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyNotShadowCaster, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "NotShadowCaster"
    }
}

#[pycomponent(NotShadowReceiver, unit, bridge)]
#[pyclass(name = "NotShadowReceiver", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyNotShadowReceiver, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "NotShadowReceiver"
    }
}

#[pycomponent(TransmittedShadowReceiver, unit, bridge)]
#[pyclass(name = "TransmittedShadowReceiver", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyTransmittedShadowReceiver, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "TransmittedShadowReceiver"
    }
}

#[pycomponent(VolumetricLight, unit, bridge)]
#[pyclass(name = "VolumetricLight", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyVolumetricLight, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "VolumetricLight"
    }
}

#[pycomponent(LightProbe, bridge)]
#[pyclass(name = "LightProbe", extends = PyComponent)]
#[derive(Debug)]
pub struct PyLightProbe {
    pub(crate) storage: ComponentStorage<LightProbe>,
}

impl PyLightProbe {
    fn default_falloff() -> PyVec3 {
        LightProbe::default().falloff.into()
    }
}

#[pymethods]
impl PyLightProbe {
    #[new]
    #[pyo3(signature = (falloff = PyLightProbe::default_falloff()))]
    pub fn new(falloff: PyVec3) -> PyClassInitializer<Self> {
        Self::from_owned(LightProbe {
            falloff: falloff.into(),
        })
        .into()
    }

    #[getter]
    pub fn falloff(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.falloff.into())
    }

    #[setter]
    pub fn set_falloff(&mut self, falloff: PyVec3) -> PyResult<()> {
        self.as_mut()?.falloff = falloff.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("LightProbe(falloff={:?})", self.as_ref()?.falloff))
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()?.falloff == other.as_ref()?.falloff)
    }
}
