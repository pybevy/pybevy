pub mod assets;
pub mod components;
pub mod label;
pub mod loader_settings;
pub mod plugin;

pub use assets::{PyGltf, PyGltfMesh, PyGltfNode, PyGltfPrimitive, PyGltfSkin};
use bevy::gltf::{
    Gltf, GltfExtras, GltfMaterialExtras, GltfMaterialName, GltfMesh, GltfMeshExtras, GltfMeshName,
    GltfNode, GltfPrimitive, GltfSceneExtras, GltfSkin,
};
pub use components::{
    PyGltfExtras, PyGltfMaterialExtras, PyGltfMaterialName, PyGltfMeshExtras, PyGltfMeshName,
    PyGltfSceneExtras,
};
pub use label::PyGltfAssetLabel;
pub use loader_settings::PyGltfLoaderSettings;
pub use plugin::PyGltfPlugin;
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{asset_bridge, component_bridge, plugin_bridge};
use pyo3::prelude::*;

component_bridge!(GltfExtras, PyGltfExtras);
component_bridge!(GltfMeshName, PyGltfMeshName);
component_bridge!(GltfMaterialName, PyGltfMaterialName);
component_bridge!(GltfSceneExtras, PyGltfSceneExtras);
component_bridge!(GltfMeshExtras, PyGltfMeshExtras);
component_bridge!(GltfMaterialExtras, PyGltfMaterialExtras);

asset_bridge!(Gltf, PyGltf);
asset_bridge!(GltfMesh, PyGltfMesh);
asset_bridge!(GltfNode, PyGltfNode);
asset_bridge!(GltfPrimitive, PyGltfPrimitive);
asset_bridge!(GltfSkin, PyGltfSkin);

plugin_bridge!(PyGltfPlugin, bevy::gltf::GltfPlugin);

pub fn register_gltf_bridges() {
    global_registry::register_component_bridge(GltfExtrasBridge);
    global_registry::register_component_bridge(GltfMeshNameBridge);
    global_registry::register_component_bridge(GltfMaterialNameBridge);
    global_registry::register_component_bridge(GltfSceneExtrasBridge);
    global_registry::register_component_bridge(GltfMeshExtrasBridge);
    global_registry::register_component_bridge(GltfMaterialExtrasBridge);

    global_registry::register_asset_bridge(GltfBridge);
    global_registry::register_asset_bridge(GltfMeshBridge);
    global_registry::register_asset_bridge(GltfNodeBridge);
    global_registry::register_asset_bridge(GltfPrimitiveBridge);
    global_registry::register_asset_bridge(GltfSkinBridge);

    plugin_registry::register_plugin_bridge(GltfPluginBridge);
}

pub fn add_gltf_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_gltf_bridges();

    m.add_class::<PyGltfPlugin>()?;
    m.add_class::<PyGltfExtras>()?;
    m.add_class::<PyGltfMeshName>()?;
    m.add_class::<PyGltfMaterialName>()?;
    m.add_class::<PyGltfSceneExtras>()?;
    m.add_class::<PyGltfMeshExtras>()?;
    m.add_class::<PyGltfMaterialExtras>()?;
    m.add_class::<PyGltf>()?;
    m.add_class::<PyGltfMesh>()?;
    m.add_class::<PyGltfNode>()?;
    m.add_class::<PyGltfPrimitive>()?;
    m.add_class::<PyGltfSkin>()?;
    m.add_class::<PyGltfAssetLabel>()?;
    m.add_class::<PyGltfLoaderSettings>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "gltf")?;
    add_gltf_classes(&m)?;
    parent.add_submodule(&m)
}
