pub mod assets;
pub mod components;
pub mod label;
pub mod loader_settings;
pub mod plugin;

pub use assets::{PyGltf, PyGltfMesh, PyGltfNode, PyGltfPrimitive, PyGltfSkin};
pub use components::{
    PyGltfExtras, PyGltfMaterialExtras, PyGltfMaterialName, PyGltfMeshExtras, PyGltfMeshName,
    PyGltfSceneExtras,
};
pub use label::PyGltfAssetLabel;
pub use loader_settings::PyGltfLoaderSettings;
pub use plugin::PyGltfPlugin;
use pyo3::prelude::*;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "gltf")?;
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
    parent.add_submodule(&m)
}
