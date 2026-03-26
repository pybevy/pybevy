pub mod dynamic_scene;
pub mod dynamic_sceneroot;
pub mod instance_id;
pub mod scene;
pub mod scene_instance_ready;
pub mod scene_spawner;
pub mod sceneroot;

use bevy::{
    ecs::message::{Message, Messages},
    prelude::*,
    scene::{DynamicScene, DynamicSceneRoot, Scene, SceneInstanceReady, SceneRoot},
};
pub use dynamic_scene::PyDynamicScene;
pub use dynamic_sceneroot::PyDynamicSceneRoot;
pub use instance_id::PyInstanceId;
use pybevy_core::{PyPlugin, plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{asset_bridge, handle_bridge, plugin_bridge, resource_bridge};
use pyo3::prelude::*;
pub use scene::PyScene;
pub use scene_instance_ready::PySceneInstanceReady;
pub use scene_spawner::PySceneSpawner;
pub use sceneroot::PySceneRoot;

asset_bridge!(Scene, PyScene);
asset_bridge!(DynamicScene, PyDynamicScene);

handle_bridge!(SceneRoot, PySceneRoot);
handle_bridge!(DynamicSceneRoot, PyDynamicSceneRoot);

resource_bridge!(
    bevy::scene::SceneSpawner,
    PySceneSpawner,
    no_insert,
    no_remove
);

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

plugin_bridge!(
    PyScenePlugin,
    bevy::scene::ScenePlugin,
    |_py_plugin, app| {
        app.add_plugins(bevy::scene::ScenePlugin);
        app.add_observer(scene_instance_ready_bridge);
        Ok(())
    }
);

pub fn register_scene_bridges() {
    global_registry::register_asset_bridge(SceneBridge);
    global_registry::register_asset_bridge(DynamicSceneBridge);
    global_registry::register_component_bridge(SceneRootBridge);
    global_registry::register_component_bridge(DynamicSceneRootBridge);
    global_registry::register_resource_bridge(SceneSpawnerBridge);
    plugin_registry::register_plugin_bridge(ScenePluginBridge);
}

pub fn add_scene_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_scene_bridges();

    m.add_class::<PyScenePlugin>()?;
    m.add_class::<PyDynamicScene>()?;
    m.add_class::<PyDynamicSceneRoot>()?;
    m.add_class::<PyInstanceId>()?;
    m.add_class::<PyScene>()?;
    m.add_class::<PySceneInstanceReady>()?;
    m.add_class::<PySceneSpawner>()?;
    m.add_class::<PySceneRoot>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "scene")?;
    add_scene_classes(&m)?;
    parent.add_submodule(&m)
}
