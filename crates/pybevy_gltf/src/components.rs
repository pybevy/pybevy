use bevy::gltf::{
    GltfExtras, GltfMaterialExtras, GltfMaterialName, GltfMeshExtras, GltfMeshName, GltfSceneExtras,
};
use pybevy_core::{ComponentStorage, PyComponent};
use pyo3::prelude::*;

/// GLTF extras component for storing arbitrary GLTF JSON data.
#[pyclass(name = "GltfExtras", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyGltfExtras {
    pub(crate) storage: ComponentStorage<GltfExtras>,
}

impl PyGltfExtras {
    pub fn from_owned(value: GltfExtras) -> (Self, PyComponent) {
        (
            PyGltfExtras {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }

    pub fn from_borrowed(storage: ComponentStorage<GltfExtras>) -> (Self, PyComponent) {
        (PyGltfExtras { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&GltfExtras> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut GltfExtras> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyGltfExtras {
    #[new]
    #[pyo3(signature = (value = String::new()))]
    pub fn new(value: String) -> (Self, PyComponent) {
        (GltfExtras { value }.into(), PyComponent)
    }

    #[getter]
    pub fn value(&self) -> PyResult<String> {
        Ok(self.as_ref()?.value.clone())
    }

    #[setter]
    pub fn set_value(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.value = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref() {
            Ok(extras) => Ok(format!("GltfExtras(value={:?})", extras.value)),
            Err(_) => Ok("GltfExtras(<invalid>)".to_string()),
        }
    }
}

impl From<GltfExtras> for PyGltfExtras {
    fn from(value: GltfExtras) -> Self {
        PyGltfExtras {
            storage: ComponentStorage::owned(value),
        }
    }
}

/// GLTF mesh name component.
#[pyclass(name = "GltfMeshName", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyGltfMeshName {
    pub(crate) storage: ComponentStorage<GltfMeshName>,
}

impl PyGltfMeshName {
    pub fn from_owned(value: GltfMeshName) -> (Self, PyComponent) {
        (
            PyGltfMeshName {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }

    pub fn from_borrowed(storage: ComponentStorage<GltfMeshName>) -> (Self, PyComponent) {
        (PyGltfMeshName { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&GltfMeshName> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut GltfMeshName> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyGltfMeshName {
    #[new]
    #[pyo3(signature = (name = String::new()))]
    pub fn new(name: String) -> (Self, PyComponent) {
        (GltfMeshName(name).into(), PyComponent)
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.as_ref()?.0.clone())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref() {
            Ok(name) => Ok(format!("GltfMeshName({:?})", name.0)),
            Err(_) => Ok("GltfMeshName(<invalid>)".to_string()),
        }
    }

    pub fn __str__(&self) -> PyResult<String> {
        Ok(self.as_ref()?.0.clone())
    }
}

impl From<GltfMeshName> for PyGltfMeshName {
    fn from(value: GltfMeshName) -> Self {
        PyGltfMeshName {
            storage: ComponentStorage::owned(value),
        }
    }
}

/// GLTF material name component.
#[pyclass(name = "GltfMaterialName", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyGltfMaterialName {
    pub(crate) storage: ComponentStorage<GltfMaterialName>,
}

impl PyGltfMaterialName {
    pub fn from_owned(value: GltfMaterialName) -> (Self, PyComponent) {
        (
            PyGltfMaterialName {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }

    pub fn from_borrowed(storage: ComponentStorage<GltfMaterialName>) -> (Self, PyComponent) {
        (PyGltfMaterialName { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&GltfMaterialName> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut GltfMaterialName> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyGltfMaterialName {
    #[new]
    #[pyo3(signature = (name = String::new()))]
    pub fn new(name: String) -> (Self, PyComponent) {
        (GltfMaterialName(name).into(), PyComponent)
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.as_ref()?.0.clone())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref() {
            Ok(name) => Ok(format!("GltfMaterialName({:?})", name.0)),
            Err(_) => Ok("GltfMaterialName(<invalid>)".to_string()),
        }
    }

    pub fn __str__(&self) -> PyResult<String> {
        Ok(self.as_ref()?.0.clone())
    }
}

impl From<GltfMaterialName> for PyGltfMaterialName {
    fn from(value: GltfMaterialName) -> Self {
        PyGltfMaterialName {
            storage: ComponentStorage::owned(value),
        }
    }
}

/// GLTF scene extras component.
#[pyclass(name = "GltfSceneExtras", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyGltfSceneExtras {
    pub(crate) storage: ComponentStorage<GltfSceneExtras>,
}

impl PyGltfSceneExtras {
    pub fn from_owned(value: GltfSceneExtras) -> (Self, PyComponent) {
        (
            PyGltfSceneExtras {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }

    pub fn from_borrowed(storage: ComponentStorage<GltfSceneExtras>) -> (Self, PyComponent) {
        (PyGltfSceneExtras { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&GltfSceneExtras> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut GltfSceneExtras> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyGltfSceneExtras {
    #[new]
    #[pyo3(signature = (value = String::new()))]
    pub fn new(value: String) -> (Self, PyComponent) {
        (GltfSceneExtras { value }.into(), PyComponent)
    }

    #[getter]
    pub fn value(&self) -> PyResult<String> {
        Ok(self.as_ref()?.value.clone())
    }

    #[setter]
    pub fn set_value(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.value = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref() {
            Ok(extras) => Ok(format!("GltfSceneExtras(value={:?})", extras.value)),
            Err(_) => Ok("GltfSceneExtras(<invalid>)".to_string()),
        }
    }
}

impl From<GltfSceneExtras> for PyGltfSceneExtras {
    fn from(value: GltfSceneExtras) -> Self {
        PyGltfSceneExtras {
            storage: ComponentStorage::owned(value),
        }
    }
}

/// GLTF mesh extras component.
#[pyclass(name = "GltfMeshExtras", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyGltfMeshExtras {
    pub(crate) storage: ComponentStorage<GltfMeshExtras>,
}

impl PyGltfMeshExtras {
    pub fn from_owned(value: GltfMeshExtras) -> (Self, PyComponent) {
        (
            PyGltfMeshExtras {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }

    pub fn from_borrowed(storage: ComponentStorage<GltfMeshExtras>) -> (Self, PyComponent) {
        (PyGltfMeshExtras { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&GltfMeshExtras> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut GltfMeshExtras> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyGltfMeshExtras {
    #[new]
    #[pyo3(signature = (value = String::new()))]
    pub fn new(value: String) -> (Self, PyComponent) {
        (GltfMeshExtras { value }.into(), PyComponent)
    }

    #[getter]
    pub fn value(&self) -> PyResult<String> {
        Ok(self.as_ref()?.value.clone())
    }

    #[setter]
    pub fn set_value(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.value = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref() {
            Ok(extras) => Ok(format!("GltfMeshExtras(value={:?})", extras.value)),
            Err(_) => Ok("GltfMeshExtras(<invalid>)".to_string()),
        }
    }
}

impl From<GltfMeshExtras> for PyGltfMeshExtras {
    fn from(value: GltfMeshExtras) -> Self {
        PyGltfMeshExtras {
            storage: ComponentStorage::owned(value),
        }
    }
}

/// GLTF material extras component.
#[pyclass(name = "GltfMaterialExtras", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyGltfMaterialExtras {
    pub(crate) storage: ComponentStorage<GltfMaterialExtras>,
}

impl PyGltfMaterialExtras {
    pub fn from_owned(value: GltfMaterialExtras) -> (Self, PyComponent) {
        (
            PyGltfMaterialExtras {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }

    pub fn from_borrowed(storage: ComponentStorage<GltfMaterialExtras>) -> (Self, PyComponent) {
        (PyGltfMaterialExtras { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&GltfMaterialExtras> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut GltfMaterialExtras> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyGltfMaterialExtras {
    #[new]
    #[pyo3(signature = (value = String::new()))]
    pub fn new(value: String) -> (Self, PyComponent) {
        (GltfMaterialExtras { value }.into(), PyComponent)
    }

    #[getter]
    pub fn value(&self) -> PyResult<String> {
        Ok(self.as_ref()?.value.clone())
    }

    #[setter]
    pub fn set_value(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.value = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref() {
            Ok(extras) => Ok(format!("GltfMaterialExtras(value={:?})", extras.value)),
            Err(_) => Ok("GltfMaterialExtras(<invalid>)".to_string()),
        }
    }
}

impl From<GltfMaterialExtras> for PyGltfMaterialExtras {
    fn from(value: GltfMaterialExtras) -> Self {
        PyGltfMaterialExtras {
            storage: ComponentStorage::owned(value),
        }
    }
}
