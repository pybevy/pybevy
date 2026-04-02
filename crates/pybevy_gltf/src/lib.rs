pub mod assets;
pub mod components;
pub mod label;
pub mod loader_settings;
pub mod plugin;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        assets::PyGltf, components::PyGltfExtras, label::PyGltfAssetLabel, plugin::PyGltfPlugin,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "gltf")?;
    m.add_class::<plugin::PyGltfPlugin>()?;
    m.add_class::<components::PyGltfExtras>()?;
    m.add_class::<components::PyGltfMeshName>()?;
    m.add_class::<components::PyGltfMaterialName>()?;
    m.add_class::<components::PyGltfSceneExtras>()?;
    m.add_class::<components::PyGltfMeshExtras>()?;
    m.add_class::<components::PyGltfMaterialExtras>()?;
    m.add_class::<assets::PyGltf>()?;
    m.add_class::<assets::PyGltfMesh>()?;
    m.add_class::<assets::PyGltfNode>()?;
    m.add_class::<assets::PyGltfPrimitive>()?;
    m.add_class::<assets::PyGltfSkin>()?;
    m.add_class::<label::PyGltfAssetLabel>()?;
    m.add_class::<loader_settings::PyGltfLoaderSettings>()?;
    parent.add_submodule(&m)
}
