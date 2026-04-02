use bevy::gltf::{Gltf, GltfMesh, GltfNode, GltfPrimitive, GltfSkin};
use pybevy_core::{AssetStorage, PyAsset, PyComponent, PyHandle};
use pybevy_macros::asset_storage;
use pybevy_transform::transform::PyTransform;
use pyo3::prelude::*;

use crate::label::PyGltfAssetLabel;

#[asset_storage(Gltf, no_clone, bridge)]
#[pyclass(name = "Gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltf {
    pub storage: AssetStorage<Gltf>,
}

#[pymethods]
impl PyGltf {
    #[getter]
    pub fn scenes(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.scenes.iter().map(PyHandle::from).collect())
    }

    #[getter]
    pub fn named_scenes(&self) -> PyResult<Vec<(String, PyHandle)>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf
            .named_scenes
            .iter()
            .map(|(name, handle)| (name.to_string(), PyHandle::from(handle)))
            .collect())
    }

    #[getter]
    pub fn meshes(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.meshes.iter().map(PyHandle::from).collect())
    }

    #[getter]
    pub fn named_meshes(&self) -> PyResult<Vec<(String, PyHandle)>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf
            .named_meshes
            .iter()
            .map(|(name, handle)| (name.to_string(), PyHandle::from(handle)))
            .collect())
    }

    #[getter]
    pub fn materials(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.materials.iter().map(PyHandle::from).collect())
    }

    #[getter]
    pub fn named_materials(&self) -> PyResult<Vec<(String, PyHandle)>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf
            .named_materials
            .iter()
            .map(|(name, handle)| (name.to_string(), PyHandle::from(handle)))
            .collect())
    }

    #[getter]
    pub fn nodes(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.nodes.iter().map(PyHandle::from).collect())
    }

    #[getter]
    pub fn named_nodes(&self) -> PyResult<Vec<(String, PyHandle)>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf
            .named_nodes
            .iter()
            .map(|(name, handle)| (name.to_string(), PyHandle::from(handle)))
            .collect())
    }

    #[getter]
    pub fn default_scene(&self) -> PyResult<Option<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.default_scene.as_ref().map(PyHandle::from))
    }

    #[getter]
    pub fn skins(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.skins.iter().map(PyHandle::from).collect())
    }

    #[getter]
    pub fn named_skins(&self) -> PyResult<Vec<(String, PyHandle)>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf
            .named_skins
            .iter()
            .map(|(name, handle)| (name.to_string(), PyHandle::from(handle)))
            .collect())
    }
}

#[asset_storage(GltfMesh, no_clone, bridge)]
#[pyclass(name = "GltfMesh", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfMesh {
    pub storage: AssetStorage<GltfMesh>,
}

#[pymethods]
impl PyGltfMesh {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[getter]
    pub fn primitives(&self, py: Python<'_>) -> PyResult<Vec<Py<PyGltfPrimitive>>> {
        let mesh = self.storage.as_ref()?;
        mesh.primitives
            .iter()
            .map(|primitive| Py::new(py, PyGltfPrimitive::from_owned(primitive.clone())))
            .collect()
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let mesh = self.storage.as_ref()?;
        Ok(mesh.extras.as_ref().map(|e| e.value.clone()))
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Mesh(self.storage.as_ref()?.index))
    }
}

#[asset_storage(GltfNode, no_clone, bridge)]
#[pyclass(name = "GltfNode", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfNode {
    pub storage: AssetStorage<GltfNode>,
}

#[pymethods]
impl PyGltfNode {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[getter]
    pub fn children(&self) -> PyResult<Vec<PyHandle>> {
        let node = self.storage.as_ref()?;
        Ok(node.children.iter().map(PyHandle::from).collect())
    }

    #[getter]
    pub fn mesh(&self) -> PyResult<Option<PyHandle>> {
        let node = self.storage.as_ref()?;
        Ok(node.mesh.as_ref().map(PyHandle::from))
    }

    #[getter]
    pub fn skin(&self) -> PyResult<Option<PyHandle>> {
        let node = self.storage.as_ref()?;
        Ok(node.skin.as_ref().map(PyHandle::from))
    }

    #[getter]
    pub fn transform(&self, py: Python<'_>) -> PyResult<Py<PyTransform>> {
        let node = self.storage.as_ref()?;
        Py::new(py, (PyTransform::from(node.transform), PyComponent))
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let node = self.storage.as_ref()?;
        Ok(node.extras.as_ref().map(|e| e.value.clone()))
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Node(self.storage.as_ref()?.index))
    }
}

#[asset_storage(GltfPrimitive, no_clone, bridge)]
#[pyclass(name = "GltfPrimitive", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfPrimitive {
    pub storage: AssetStorage<GltfPrimitive>,
}

#[pymethods]
impl PyGltfPrimitive {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[getter]
    pub fn parent_mesh_index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.parent_mesh_index)
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[getter]
    pub fn mesh(&self) -> PyResult<PyHandle> {
        let primitive = self.storage.as_ref()?;
        Ok(PyHandle::from(&primitive.mesh))
    }

    #[getter]
    pub fn material(&self) -> PyResult<Option<PyHandle>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.material.as_ref().map(PyHandle::from))
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.extras.as_ref().map(|e| e.value.clone()))
    }

    #[getter]
    pub fn material_extras(&self) -> PyResult<Option<String>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.material_extras.as_ref().map(|e| e.value.clone()))
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        let prim = self.storage.as_ref()?;
        Ok(PyGltfAssetLabel::Primitive {
            mesh: prim.parent_mesh_index,
            primitive: prim.index,
        })
    }
}

#[asset_storage(GltfSkin, no_clone, bridge)]
#[pyclass(name = "GltfSkin", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfSkin {
    pub storage: AssetStorage<GltfSkin>,
}

#[pymethods]
impl PyGltfSkin {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[getter]
    pub fn joints(&self) -> PyResult<Vec<PyHandle>> {
        let skin = self.storage.as_ref()?;
        Ok(skin.joints.iter().map(PyHandle::from).collect())
    }

    #[getter]
    pub fn inverse_bind_matrices(&self) -> PyResult<PyHandle> {
        let skin = self.storage.as_ref()?;
        Ok(PyHandle::from(&skin.inverse_bind_matrices))
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let skin = self.storage.as_ref()?;
        Ok(skin.extras.as_ref().map(|e| e.value.clone()))
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Skin(self.storage.as_ref()?.index))
    }
}
