use bevy::{
    image::Image,
    pbr::{MeshMaterial3d, StandardMaterial},
};
use pybevy_core::{
    AssetStorage, NativeAsset, PyAsset, PyComponent, PyHandle, PyPlugin, extract_handle_from_any,
};
use pybevy_macros::asset_storage;
use pyo3::{
    exceptions::{PyIndexError, PyTypeError},
    prelude::*,
};

use crate::{
    PyStandardMaterial,
    shader_material::{MAX_TEXTURE_SLOTS, ShaderMaterial, ShaderMaterialExtension, ShaderParams},
};

// ── ShaderMaterial asset wrapper ──────────────────────────────────────

#[asset_storage(ShaderMaterial)]
#[pyclass(name = "ShaderMaterial", extends = PyAsset)]
#[derive(Debug)]
pub struct PyShaderMaterial {
    pub storage: AssetStorage<ShaderMaterial>,
}

#[pymethods]
impl PyShaderMaterial {
    #[new]
    #[pyo3(signature = (base, fragment_shader=None, vertex_shader=None, data=None, shader_defs=0, shader_def_names=None, textures=None, bindings_wgsl=None))]
    pub fn new(
        py: Python<'_>,
        base: &Bound<'_, PyAny>,
        fragment_shader: Option<String>,
        vertex_shader: Option<String>,
        data: Option<Vec<f32>>,
        shader_defs: u32,
        shader_def_names: Option<Vec<String>>,
        textures: Option<Py<pyo3::types::PyList>>,
        bindings_wgsl: Option<String>,
    ) -> PyResult<(Self, PyAsset)> {
        let mut py_mat = base.extract::<PyRefMut<'_, PyStandardMaterial>>()?;
        let std_mat: StandardMaterial = py_mat.take()?;

        let mut params = ShaderParams::default();

        if let Some(data) = data {
            if data.len() > 256 {
                return Err(PyTypeError::new_err(format!(
                    "data has {} floats, max is 256 (64 vec4s)",
                    data.len()
                )));
            }
            for (i, &val) in data.iter().enumerate() {
                let vec_idx = i / 4;
                let comp_idx = i % 4;
                params.data[vec_idx][comp_idx] = val;
            }
        }

        // Extract texture handles from Python list [handle_or_None, ...]
        let mut tex_handles: [Option<bevy::asset::Handle<Image>>; MAX_TEXTURE_SLOTS] =
            Default::default();

        if let Some(tex_list) = textures {
            let list = tex_list.bind(py);
            for (i, item) in list.iter().enumerate() {
                if i >= MAX_TEXTURE_SLOTS {
                    return Err(PyIndexError::new_err(format!(
                        "texture slot {} exceeds max ({})",
                        i, MAX_TEXTURE_SLOTS
                    )));
                }
                if !item.is_none() {
                    let py_handle = extract_handle_from_any(&item)?;
                    let typed: bevy::asset::Handle<Image> = (&py_handle).try_into()?;
                    tex_handles[i] = Some(typed);
                }
            }
        }

        let material = ShaderMaterial {
            base: std_mat,
            extension: ShaderMaterialExtension {
                params,
                texture_0: tex_handles[0].take(),
                texture_1: tex_handles[1].take(),
                texture_2: tex_handles[2].take(),
                texture_3: tex_handles[3].take(),
                fragment_shader_path: fragment_shader,
                vertex_shader_path: vertex_shader,
                shader_defs,
                shader_def_names: shader_def_names.unwrap_or_default(),
                bindings_wgsl,
            },
        };
        Ok(Self::from_owned(material))
    }

    /// Get the shader defs bitmask.
    fn get_shader_defs(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.extension.shader_defs)
    }

    /// Set the shader defs bitmask.
    fn set_shader_defs(&mut self, defs: u32) -> PyResult<()> {
        self.as_mut()?.extension.shader_defs = defs;
        Ok(())
    }

    /// Set a texture handle for a given slot (0-3).
    fn set_texture(&mut self, slot: usize, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_handle = extract_handle_from_any(handle)?;
        let typed: bevy::asset::Handle<Image> = (&py_handle).try_into()?;
        let mat = self.as_mut()?;
        match slot {
            0 => mat.extension.texture_0 = Some(typed),
            1 => mat.extension.texture_1 = Some(typed),
            2 => mat.extension.texture_2 = Some(typed),
            3 => mat.extension.texture_3 = Some(typed),
            _ => {
                return Err(PyIndexError::new_err(format!(
                    "texture slot {} out of range (max {})",
                    slot,
                    MAX_TEXTURE_SLOTS - 1
                )));
            }
        }
        Ok(())
    }

    /// Clear a texture slot (set to None).
    fn clear_texture(&mut self, slot: usize) -> PyResult<()> {
        let mat = self.as_mut()?;
        match slot {
            0 => mat.extension.texture_0 = None,
            1 => mat.extension.texture_1 = None,
            2 => mat.extension.texture_2 = None,
            3 => mat.extension.texture_3 = None,
            _ => {
                return Err(PyIndexError::new_err(format!(
                    "texture slot {} out of range (max {})",
                    slot,
                    MAX_TEXTURE_SLOTS - 1
                )));
            }
        }
        Ok(())
    }

    /// Set a single float in the uniform data buffer.
    /// Index is the float index (0-255), not byte offset.
    fn set_data_float(&mut self, index: usize, value: f32) -> PyResult<()> {
        if index >= 256 {
            return Err(PyIndexError::new_err(format!(
                "float index {} out of range (max 255)",
                index
            )));
        }
        let mat = self.as_mut()?;
        mat.extension.params.data[index / 4][index % 4] = value;
        Ok(())
    }

    /// Get a single float from the uniform data buffer.
    fn get_data_float(&self, index: usize) -> PyResult<f32> {
        if index >= 256 {
            return Err(PyIndexError::new_err(format!(
                "float index {} out of range (max 255)",
                index
            )));
        }
        let mat = self.as_ref()?;
        Ok(mat.extension.params.data[index / 4][index % 4])
    }

    /// Set multiple floats starting at the given index.
    fn set_data_floats(&mut self, start: usize, values: Vec<f32>) -> PyResult<()> {
        let end = start + values.len();
        if end > 256 {
            return Err(PyIndexError::new_err(format!(
                "float range {}..{} exceeds buffer (max 256)",
                start, end
            )));
        }
        let mat = self.as_mut()?;
        for (i, &val) in values.iter().enumerate() {
            let idx = start + i;
            mat.extension.params.data[idx / 4][idx % 4] = val;
        }
        Ok(())
    }

    /// Get multiple floats starting at the given index.
    fn get_data_floats(&self, start: usize, count: usize) -> PyResult<Vec<f32>> {
        let end = start + count;
        if end > 256 {
            return Err(PyIndexError::new_err(format!(
                "float range {}..{} exceeds buffer (max 256)",
                start, end
            )));
        }
        let mat = self.as_ref()?;
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            let idx = start + i;
            result.push(mat.extension.params.data[idx / 4][idx % 4]);
        }
        Ok(result)
    }
}

// ── ShaderMaterialPlugin ──────────────────────────────────────────────

#[pyclass(name = "ShaderMaterialPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone)]
pub struct PyShaderMaterialPlugin;

#[pymethods]
impl PyShaderMaterialPlugin {
    #[new]
    #[pyo3(signature = ())]
    pub fn new() -> (Self, PyPlugin) {
        (PyShaderMaterialPlugin, PyPlugin)
    }
}

// ── MeshMaterial3dShader component ────────────────────────────────────

#[pyclass(name = "MeshMaterial3dShader", extends = PyComponent, eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMeshMaterial3dShader(pub(crate) PyHandle);

impl TryFrom<&PyMeshMaterial3dShader> for MeshMaterial3d<ShaderMaterial> {
    type Error = PyErr;

    fn try_from(value: &PyMeshMaterial3dShader) -> Result<Self, Self::Error> {
        Ok(MeshMaterial3d((&value.0).try_into()?))
    }
}

impl From<&MeshMaterial3d<ShaderMaterial>> for PyMeshMaterial3dShader {
    fn from(value: &MeshMaterial3d<ShaderMaterial>) -> Self {
        PyMeshMaterial3dShader((&value.0).into())
    }
}

#[pymethods]
impl PyMeshMaterial3dShader {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(handle)?;

        // Validate asset type
        if let Some(name) = handle.asset_type_name() {
            if name != "ShaderMaterial" {
                return Err(PyTypeError::new_err(format!(
                    "AssetType `{}` does not match expected type `ShaderMaterial`",
                    name
                )));
            }
        }

        Ok((Self(handle), PyComponent))
    }

    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
