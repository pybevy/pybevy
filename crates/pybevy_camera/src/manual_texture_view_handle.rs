use bevy::camera::ManualTextureViewHandle;
use pyo3::prelude::*;

#[pyclass(
    name = "ManualTextureViewHandle",
    module = "pybevy.camera",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyManualTextureViewHandle(pub(crate) ManualTextureViewHandle);

impl From<ManualTextureViewHandle> for PyManualTextureViewHandle {
    fn from(value: ManualTextureViewHandle) -> Self {
        Self(value)
    }
}

impl From<PyManualTextureViewHandle> for ManualTextureViewHandle {
    fn from(value: PyManualTextureViewHandle) -> Self {
        value.0
    }
}

#[pymethods]
impl PyManualTextureViewHandle {
    #[new]
    pub fn new(value: u32) -> Self {
        Self(ManualTextureViewHandle(value))
    }

    #[getter]
    pub fn value(&self) -> u32 {
        self.0.0
    }

    fn __repr__(&self) -> String {
        format!("ManualTextureViewHandle({})", self.0.0)
    }
}
