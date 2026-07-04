use bevy::gltf::{
    GltfExtras, GltfMaterialExtras, GltfMaterialName, GltfMeshExtras, GltfMeshName, GltfSceneExtras,
};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(GltfExtras, bridge)]
#[pyclass(name = "GltfExtras", extends = PyComponent)]
#[derive(Debug)]
pub struct PyGltfExtras {
    pub(crate) storage: ComponentStorage<GltfExtras>,
}

#[pymethods]
impl PyGltfExtras {
    #[new]
    #[pyo3(signature = (value = String::new()))]
    pub fn new(value: String) -> PyClassInitializer<Self> {
        (GltfExtras { value }.into(), PyComponent).into()
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

#[pycomponent(GltfMeshName, bridge)]
#[pyclass(name = "GltfMeshName", extends = PyComponent)]
#[derive(Debug)]
pub struct PyGltfMeshName {
    pub(crate) storage: ComponentStorage<GltfMeshName>,
}

#[pymethods]
impl PyGltfMeshName {
    #[new]
    #[pyo3(signature = (name = String::new()))]
    pub fn new(name: String) -> PyClassInitializer<Self> {
        (GltfMeshName(name).into(), PyComponent).into()
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

#[pycomponent(GltfMaterialName, bridge)]
#[pyclass(name = "GltfMaterialName", extends = PyComponent)]
#[derive(Debug)]
pub struct PyGltfMaterialName {
    pub(crate) storage: ComponentStorage<GltfMaterialName>,
}

#[pymethods]
impl PyGltfMaterialName {
    #[new]
    #[pyo3(signature = (name = String::new()))]
    pub fn new(name: String) -> PyClassInitializer<Self> {
        (GltfMaterialName(name).into(), PyComponent).into()
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

#[pycomponent(GltfSceneExtras, bridge)]
#[pyclass(name = "GltfSceneExtras", extends = PyComponent)]
#[derive(Debug)]
pub struct PyGltfSceneExtras {
    pub(crate) storage: ComponentStorage<GltfSceneExtras>,
}

#[pymethods]
impl PyGltfSceneExtras {
    #[new]
    #[pyo3(signature = (value = String::new()))]
    pub fn new(value: String) -> PyClassInitializer<Self> {
        (GltfSceneExtras { value }.into(), PyComponent).into()
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

#[pycomponent(GltfMeshExtras, bridge)]
#[pyclass(name = "GltfMeshExtras", extends = PyComponent)]
#[derive(Debug)]
pub struct PyGltfMeshExtras {
    pub(crate) storage: ComponentStorage<GltfMeshExtras>,
}

#[pymethods]
impl PyGltfMeshExtras {
    #[new]
    #[pyo3(signature = (value = String::new()))]
    pub fn new(value: String) -> PyClassInitializer<Self> {
        (GltfMeshExtras { value }.into(), PyComponent).into()
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

#[pycomponent(GltfMaterialExtras, bridge)]
#[pyclass(name = "GltfMaterialExtras", extends = PyComponent)]
#[derive(Debug)]
pub struct PyGltfMaterialExtras {
    pub(crate) storage: ComponentStorage<GltfMaterialExtras>,
}

#[pymethods]
impl PyGltfMaterialExtras {
    #[new]
    #[pyo3(signature = (value = String::new()))]
    pub fn new(value: String) -> PyClassInitializer<Self> {
        (GltfMaterialExtras { value }.into(), PyComponent).into()
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
