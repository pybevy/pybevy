pub mod dynamic_scene;
pub mod dynamic_sceneroot;
pub mod instance_id;
pub mod scene;
pub mod scene_instance_ready;
pub mod scene_spawner;
pub mod sceneroot;

use bevy::{
    app::App,
    ecs::message::{Message, Messages},
    prelude::*,
    scene::SceneInstanceReady,
};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        PyScenePlugin, dynamic_scene::PyDynamicScene, dynamic_sceneroot::PyDynamicSceneRoot,
        scene::PyScene, scene_spawner::PySceneSpawner, sceneroot::PySceneRoot,
    };
}

#[derive(Clone, Debug)]
pub struct SceneInstanceReadyMessage(pub SceneInstanceReady);

impl Message for SceneInstanceReadyMessage {}

pub fn scene_instance_ready_bridge(trigger: On<SceneInstanceReady>, mut commands: Commands) {
    let event = trigger.event();
    let event_clone = *event;
    commands.queue(move |world: &mut World| {
        let mut messages =
            world.get_resource_or_insert_with(Messages::<SceneInstanceReadyMessage>::default);
        messages.write(SceneInstanceReadyMessage(event_clone));
    });
}

#[plugin_storage(bevy::scene::ScenePlugin)]
#[pyclass(name = "ScenePlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyScenePlugin;

#[pymethods]
impl PyScenePlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyScenePlugin, PyPlugin)
    }
}

impl PluginBuild for PyScenePlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(bevy::scene::ScenePlugin);
        app.add_observer(scene_instance_ready_bridge);
        Ok(())
    }
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "scene")?;
    m.add_class::<PyScenePlugin>()?;
    m.add_class::<dynamic_scene::PyDynamicScene>()?;
    m.add_class::<dynamic_sceneroot::PyDynamicSceneRoot>()?;
    m.add_class::<instance_id::PyInstanceId>()?;
    m.add_class::<scene::PyScene>()?;
    m.add_class::<scene_instance_ready::PySceneInstanceReady>()?;
    m.add_class::<scene_spawner::PySceneSpawner>()?;
    m.add_class::<sceneroot::PySceneRoot>()?;
    parent.add_submodule(&m)
}
