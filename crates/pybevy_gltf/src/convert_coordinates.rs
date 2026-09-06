use bevy::gltf::convert_coordinates::GltfConvertCoordinates;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

#[pyvalue]
#[pyclass(
    name = "GltfConvertCoordinates",
    module = "pybevy.gltf",
    from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyGltfConvertCoordinates {
    pub(crate) storage: ValueStorage<GltfConvertCoordinates>,
}

impl From<GltfConvertCoordinates> for PyGltfConvertCoordinates {
    fn from(value: GltfConvertCoordinates) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyGltfConvertCoordinates> for GltfConvertCoordinates {
    type Error = PyErr;

    fn try_from(value: PyGltfConvertCoordinates) -> PyResult<Self> {
        Ok(value.storage.get()?)
    }
}

impl TryFrom<&PyGltfConvertCoordinates> for GltfConvertCoordinates {
    type Error = PyErr;

    fn try_from(value: &PyGltfConvertCoordinates) -> PyResult<Self> {
        Ok(value.storage.get()?)
    }
}

#[pymethods]
impl PyGltfConvertCoordinates {
    #[new]
    #[pyo3(signature = (rotate_scene_entity = false, rotate_meshes = false))]
    pub fn new(rotate_scene_entity: bool, rotate_meshes: bool) -> Self {
        Self::from_owned(GltfConvertCoordinates {
            rotate_scene_entity,
            rotate_meshes,
        })
    }

    #[getter]
    pub fn rotate_scene_entity(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.rotate_scene_entity)
    }

    #[setter]
    pub fn set_rotate_scene_entity(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.rotate_scene_entity = value;
        Ok(())
    }

    #[getter]
    pub fn rotate_meshes(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.rotate_meshes)
    }

    #[setter]
    pub fn set_rotate_meshes(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.rotate_meshes = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let value = self.as_ref()?;
        Ok(format!(
            "GltfConvertCoordinates(rotate_scene_entity={}, rotate_meshes={})",
            value.rotate_scene_entity, value.rotate_meshes
        ))
    }
}
