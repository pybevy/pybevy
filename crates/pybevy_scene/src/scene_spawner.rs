use bevy::{
    asset::Handle,
    scene::{DynamicScene, Scene, SceneSpawner},
};
use pybevy_core::{PyEntity, PyHandle, PyResource, ResourceStorage};
use pybevy_macros::resource_storage;
use pyo3::prelude::*;

use crate::instance_id::PyInstanceId;

#[resource_storage(SceneSpawner, no_clone, bridge, no_insert, no_remove)]
#[pyclass(name = "SceneSpawner", extends = PyResource)]
pub struct PySceneSpawner {
    pub(crate) storage: ResourceStorage<SceneSpawner>,
}

#[pymethods]
impl PySceneSpawner {
    pub fn spawn_dynamic(&mut self, id: &PyHandle) -> PyResult<PyInstanceId> {
        let handle: Handle<DynamicScene> = id.try_into()?;
        let instance_id = self.as_mut()?.spawn_dynamic(handle);
        Ok(PyInstanceId(instance_id))
    }

    pub fn spawn_dynamic_as_child(
        &mut self,
        id: &PyHandle,
        parent: &PyEntity,
    ) -> PyResult<PyInstanceId> {
        let handle: Handle<DynamicScene> = id.try_into()?;
        let instance_id = self.as_mut()?.spawn_dynamic_as_child(handle, parent.0);
        Ok(PyInstanceId(instance_id))
    }

    pub fn spawn(&mut self, id: &PyHandle) -> PyResult<PyInstanceId> {
        let handle: Handle<Scene> = id.try_into()?;
        let instance_id = self.as_mut()?.spawn(handle);
        Ok(PyInstanceId(instance_id))
    }

    pub fn spawn_as_child(&mut self, id: &PyHandle, parent: &PyEntity) -> PyResult<PyInstanceId> {
        let handle: Handle<Scene> = id.try_into()?;
        let instance_id = self.as_mut()?.spawn_as_child(handle, parent.0);
        Ok(PyInstanceId(instance_id))
    }

    pub fn despawn(&mut self, id: &PyHandle) -> PyResult<()> {
        let handle: Handle<Scene> = id.try_into()?;
        self.as_mut()?.despawn(&handle);
        Ok(())
    }

    pub fn despawn_dynamic(&mut self, id: &PyHandle) -> PyResult<()> {
        let handle: Handle<DynamicScene> = id.try_into()?;
        self.as_mut()?.despawn_dynamic(&handle);
        Ok(())
    }

    pub fn despawn_instance(&mut self, instance_id: &PyInstanceId) -> PyResult<()> {
        self.as_mut()?.despawn_instance(instance_id.0);
        Ok(())
    }

    pub fn unregister_instance(&mut self, instance_id: &PyInstanceId) -> PyResult<()> {
        self.as_mut()?.unregister_instance(instance_id.0);
        Ok(())
    }

    pub fn instance_is_ready(&self, instance_id: &PyInstanceId) -> PyResult<bool> {
        Ok(self.as_ref()?.instance_is_ready(instance_id.0))
    }

    pub fn iter_instance_entities(
        &self,
        instance_id: &PyInstanceId,
    ) -> PyResult<Option<Vec<PyEntity>>> {
        let spawner = self.as_ref()?;
        if !spawner.instance_is_ready(instance_id.0) {
            return Ok(None);
        }
        Ok(Some(
            spawner
                .iter_instance_entities(instance_id.0)
                .map(PyEntity)
                .collect(),
        ))
    }
}
