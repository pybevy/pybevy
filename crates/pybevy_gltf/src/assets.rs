use std::collections::HashMap;

use bevy::{
    asset::{Asset, Handle},
    gltf::{Gltf, GltfExtras, GltfMaterial, GltfMesh, GltfNode, GltfPrimitive, GltfSkin},
    platform::collections::HashMap as BevyHashMap,
};
use pybevy_core::{AssetStorage, PyAsset, PyHandle};
use pybevy_macros::pyasset;
use pybevy_transform::transform::PyTransform;
use pyo3::prelude::*;

use crate::{gltf_primitives::PyGltfPrimitives, label::PyGltfAssetLabel};

fn extract_handle<A: Asset>(handle: PyHandle) -> PyResult<Handle<A>> {
    handle.try_into()
}

fn extract_handles<A: Asset>(handles: Vec<PyHandle>) -> PyResult<Vec<Handle<A>>> {
    handles.into_iter().map(extract_handle).collect()
}

fn extract_named_handles<A: Asset, I>(handles: I) -> PyResult<BevyHashMap<Box<str>, Handle<A>>>
where
    I: IntoIterator<Item = (String, PyHandle)>,
{
    handles
        .into_iter()
        .map(|(name, handle)| Ok((name.into_boxed_str(), extract_handle(handle)?)))
        .collect()
}

#[pyasset(GltfMaterial, no_clone, bridge)]
#[pyclass(name = "GltfMaterial", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfMaterial {
    pub(crate) storage: AssetStorage<GltfMaterial>,
}

#[pyasset(Gltf, no_clone, bridge)]
#[pyclass(name = "Gltf", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltf {
    pub(crate) storage: AssetStorage<Gltf>,
}

#[pymethods]
impl PyGltf {
    #[getter]
    pub fn scenes(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.scenes.iter().map(PyHandle::from).collect())
    }

    #[setter]
    pub fn set_scenes(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.scenes = extract_handles(value)?;
        Ok(())
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

    #[setter]
    pub fn set_named_scenes(&mut self, value: Vec<(String, PyHandle)>) -> PyResult<()> {
        self.as_mut()?.named_scenes = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn meshes(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.meshes.iter().map(PyHandle::from).collect())
    }

    #[setter]
    pub fn set_meshes(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.meshes = extract_handles(value)?;
        Ok(())
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

    #[setter]
    pub fn set_named_meshes(&mut self, value: Vec<(String, PyHandle)>) -> PyResult<()> {
        self.as_mut()?.named_meshes = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn materials(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.materials.iter().map(PyHandle::from).collect())
    }

    #[setter]
    pub fn set_materials(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.materials = extract_handles(value)?;
        Ok(())
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

    #[setter]
    pub fn set_named_materials(&mut self, value: Vec<(String, PyHandle)>) -> PyResult<()> {
        self.as_mut()?.named_materials = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn nodes(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.nodes.iter().map(PyHandle::from).collect())
    }

    #[setter]
    pub fn set_nodes(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.nodes = extract_handles(value)?;
        Ok(())
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

    #[setter]
    pub fn set_named_nodes(&mut self, value: Vec<(String, PyHandle)>) -> PyResult<()> {
        self.as_mut()?.named_nodes = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn default_scene(&self) -> PyResult<Option<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.default_scene.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_default_scene(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.default_scene = value.map(extract_handle).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn skins(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.skins.iter().map(PyHandle::from).collect())
    }

    #[setter]
    pub fn set_skins(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.skins = extract_handles(value)?;
        Ok(())
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

    #[setter]
    pub fn set_named_skins(&mut self, value: Vec<(String, PyHandle)>) -> PyResult<()> {
        self.as_mut()?.named_skins = extract_named_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn animations(&self) -> PyResult<Vec<PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf.animations.iter().map(PyHandle::from).collect())
    }

    #[setter]
    pub fn set_animations(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.animations = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn named_animations(&self) -> PyResult<HashMap<String, PyHandle>> {
        let gltf = self.storage.as_ref()?;
        Ok(gltf
            .named_animations
            .iter()
            .map(|(name, handle)| (name.to_string(), PyHandle::from(handle)))
            .collect())
    }

    #[setter]
    pub fn set_named_animations(&mut self, value: HashMap<String, PyHandle>) -> PyResult<()> {
        self.as_mut()?.named_animations = extract_named_handles(value)?;
        Ok(())
    }
}

#[pyasset(GltfMesh, no_clone, bridge)]
#[pyclass(name = "GltfMesh", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfMesh {
    pub(crate) storage: AssetStorage<GltfMesh>,
}

#[pymethods]
impl PyGltfMesh {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.index = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn primitives(&self) -> PyResult<PyGltfPrimitives> {
        Ok(self
            .storage
            .borrow_field_as(|mesh| &mesh.primitives, |mesh| &mut mesh.primitives)?)
    }

    #[setter]
    pub fn set_primitives(&mut self, value: Vec<PyRef<'_, PyGltfPrimitive>>) -> PyResult<()> {
        let primitives = value
            .into_iter()
            .map(|primitive| Ok(PyGltfPrimitive::as_ref(&primitive)?.clone()))
            .collect::<PyResult<Vec<_>>>()?;
        self.as_mut()?.primitives = primitives;
        Ok(())
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let mesh = self.storage.as_ref()?;
        Ok(mesh.extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Mesh {
            index: self.storage.as_ref()?.index,
        })
    }
}

#[pyasset(GltfNode, no_clone, bridge)]
#[pyclass(name = "GltfNode", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfNode {
    pub(crate) storage: AssetStorage<GltfNode>,
}

#[pymethods]
impl PyGltfNode {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.index = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn children(&self) -> PyResult<Vec<PyHandle>> {
        let node = self.storage.as_ref()?;
        Ok(node.children.iter().map(PyHandle::from).collect())
    }

    #[setter]
    pub fn set_children(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.children = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn mesh(&self) -> PyResult<Option<PyHandle>> {
        let node = self.storage.as_ref()?;
        Ok(node.mesh.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_mesh(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.mesh = value.map(extract_handle).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn skin(&self) -> PyResult<Option<PyHandle>> {
        let node = self.storage.as_ref()?;
        Ok(node.skin.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_skin(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.skin = value.map(extract_handle).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn transform(&self, py: Python<'_>) -> PyResult<Py<PyTransform>> {
        let storage = self
            .storage
            .borrow_field(|node| &node.transform, |node| &mut node.transform)?;
        Py::new(py, PyTransform::from_borrowed(storage))
    }

    #[setter]
    pub fn set_transform(&mut self, value: PyRef<'_, PyTransform>) -> PyResult<()> {
        self.as_mut()?.transform = *PyTransform::as_ref(&value)?;
        Ok(())
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let node = self.storage.as_ref()?;
        Ok(node.extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Node {
            index: self.storage.as_ref()?.index,
        })
    }
}

#[pyasset(GltfPrimitive, no_clone, bridge)]
#[pyclass(name = "GltfPrimitive", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfPrimitive {
    pub(crate) storage: AssetStorage<GltfPrimitive>,
}

#[pymethods]
impl PyGltfPrimitive {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.index = value;
        Ok(())
    }

    #[getter]
    pub fn parent_mesh_index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.parent_mesh_index)
    }

    #[setter]
    pub fn set_parent_mesh_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.parent_mesh_index = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn mesh(&self) -> PyResult<PyHandle> {
        let primitive = self.storage.as_ref()?;
        Ok(PyHandle::from(&primitive.mesh))
    }

    #[setter]
    pub fn set_mesh(&mut self, value: PyHandle) -> PyResult<()> {
        self.as_mut()?.mesh = extract_handle(value)?;
        Ok(())
    }

    #[getter]
    pub fn material(&self) -> PyResult<Option<PyHandle>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.material.as_ref().map(PyHandle::from))
    }

    #[setter]
    pub fn set_material(&mut self, value: Option<PyHandle>) -> PyResult<()> {
        self.as_mut()?.material = value.map(extract_handle).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    #[getter]
    pub fn material_extras(&self) -> PyResult<Option<String>> {
        let primitive = self.storage.as_ref()?;
        Ok(primitive.material_extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_material_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.material_extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        let prim = self.storage.as_ref()?;
        Ok(PyGltfAssetLabel::Primitive {
            mesh: prim.parent_mesh_index,
            primitive: prim.index,
        })
    }
}

#[pyasset(GltfSkin, no_clone, bridge)]
#[pyclass(name = "GltfSkin", module = "pybevy.gltf", extends = PyAsset)]
#[derive(Debug)]
pub struct PyGltfSkin {
    pub(crate) storage: AssetStorage<GltfSkin>,
}

#[pymethods]
impl PyGltfSkin {
    #[getter]
    pub fn index(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.index)
    }

    #[setter]
    pub fn set_index(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.index = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.storage.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn joints(&self) -> PyResult<Vec<PyHandle>> {
        let skin = self.storage.as_ref()?;
        Ok(skin.joints.iter().map(PyHandle::from).collect())
    }

    #[setter]
    pub fn set_joints(&mut self, value: Vec<PyHandle>) -> PyResult<()> {
        self.as_mut()?.joints = extract_handles(value)?;
        Ok(())
    }

    #[getter]
    pub fn inverse_bind_matrices(&self) -> PyResult<PyHandle> {
        let skin = self.storage.as_ref()?;
        Ok(PyHandle::from(&skin.inverse_bind_matrices))
    }

    #[setter]
    pub fn set_inverse_bind_matrices(&mut self, value: PyHandle) -> PyResult<()> {
        self.as_mut()?.inverse_bind_matrices = extract_handle(value)?;
        Ok(())
    }

    #[getter]
    pub fn extras(&self) -> PyResult<Option<String>> {
        let skin = self.storage.as_ref()?;
        Ok(skin.extras.as_ref().map(|e| e.value.clone()))
    }

    #[setter]
    pub fn set_extras(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.extras = value.map(|value| GltfExtras { value });
        Ok(())
    }

    pub fn asset_label(&self) -> PyResult<PyGltfAssetLabel> {
        Ok(PyGltfAssetLabel::Skin {
            index: self.storage.as_ref()?.index,
        })
    }
}
