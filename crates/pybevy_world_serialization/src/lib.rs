pub mod dynamic_world;
pub mod dynamic_world_root;
pub mod instance_id;
pub mod world_asset;
pub mod world_asset_root;
pub mod world_instance_ready;
pub mod world_instance_spawner;

use bevy::{
    app::App,
    ecs::message::{Message, Messages},
    prelude::*,
    world_serialization::WorldInstanceReady,
};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        PyWorldSerializationPlugin, dynamic_world::PyDynamicWorld,
        dynamic_world_root::PyDynamicWorldRoot, world_asset::PyWorldAsset,
        world_asset_root::PyWorldAssetRoot, world_instance_spawner::PyWorldInstanceSpawner,
    };
}

#[derive(Clone, Debug)]
pub struct WorldInstanceReadyMessage(pub WorldInstanceReady);

impl Message for WorldInstanceReadyMessage {}

pub fn world_instance_ready_bridge(trigger: On<WorldInstanceReady>, mut commands: Commands) {
    let event = trigger.event();
    let event_clone = *event;
    commands.queue(move |world: &mut World| {
        let mut messages =
            world.get_resource_or_insert_with(Messages::<WorldInstanceReadyMessage>::default);
        messages.write(WorldInstanceReadyMessage(event_clone));
    });
}

#[pyplugin(bevy::world_serialization::WorldSerializationPlugin)]
#[pyclass(name = "WorldSerializationPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyWorldSerializationPlugin;

#[pymethods]
impl PyWorldSerializationPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyWorldSerializationPlugin, PyPlugin)
    }
}

impl PluginBuild for PyWorldSerializationPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(bevy::world_serialization::WorldSerializationPlugin);
        app.add_observer(world_instance_ready_bridge);
        Ok(())
    }
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "world_serialization")?;
    m.add_class::<PyWorldSerializationPlugin>()?;
    m.add_class::<dynamic_world::PyDynamicWorld>()?;
    m.add_class::<dynamic_world_root::PyDynamicWorldRoot>()?;
    m.add_class::<instance_id::PyInstanceId>()?;
    m.add_class::<world_asset::PyWorldAsset>()?;
    m.add_class::<world_asset_root::PyWorldAssetRoot>()?;
    m.add_class::<world_instance_ready::PyWorldInstanceReady>()?;
    m.add_class::<world_instance_spawner::PyWorldInstanceSpawner>()?;
    parent.add_submodule(&m)
}
