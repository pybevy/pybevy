use bevy::{
    app::{App, Last},
    asset::Handle,
    image::Image,
    pbr::{MaterialPlugin, MeshMaterial3d, StandardMaterial},
};
use pybevy_core::{
    AssetInputConverter, AssetStorage, ComponentStorage, LogicalTypeId, NativeAsset, PluginBuild,
    PyComponent, PyHandle, PyMaterial, PyPlugin, ensure_asset_type, extract_handle_from_any,
    public_error::{
        SHADER_DEFS_WITHOUT_NAMES, expected_float_sequence, non_negative_argument,
        too_many_shader_def_names,
    },
};
use pybevy_macros::{pyasset, pycomponent, pyplugin};
use pyo3::{
    exceptions::{PyIndexError, PyTypeError, PyValueError},
    prelude::*,
    types::PyList,
};

use crate::{
    shader_material::{MAX_TEXTURE_SLOTS, ShaderMaterial, ShaderMaterialExtension, ShaderParams},
    standard_material::PyStandardMaterial,
};

#[pyasset(ShaderMaterial, bridge, not_loadable, input_converter, material)]
#[pyclass(name = "ShaderMaterial", extends = PyMaterial, skip_from_py_object)]
#[derive(Debug)]
pub struct PyShaderMaterial {
    pub storage: AssetStorage<ShaderMaterial>,
}

impl AssetInputConverter for PyShaderMaterial {
    fn try_convert_input<'py>(
        asset: &Bound<'py, PyAny>,
        _py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        // `to_shader_material` is the `@material` decorator's auto-conversion hook.
        if let Ok(converter) = asset.getattr("to_shader_material") {
            return Ok(Some(converter.call0()?));
        }
        Ok(None)
    }
}

fn checked_index(value: isize, name: &str) -> PyResult<usize> {
    usize::try_from(value).map_err(|_| PyIndexError::new_err(non_negative_argument(name, value)))
}

fn validate_shader_defs(shader_defs: u32, shader_def_names: &[String]) -> PyResult<()> {
    let name_count = shader_def_names.len();
    if name_count > u32::BITS as usize {
        return Err(PyValueError::new_err(too_many_shader_def_names(name_count)));
    }
    let named_bits = match name_count {
        0 => 0,
        count if count == u32::BITS as usize => u32::MAX,
        count => (1_u32 << count) - 1,
    };
    if shader_defs & !named_bits != 0 {
        return Err(PyValueError::new_err(SHADER_DEFS_WITHOUT_NAMES));
    }
    Ok(())
}

#[pymethods]
impl PyShaderMaterial {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (base, fragment_shader=None, vertex_shader=None, data=None, shader_defs=0, shader_def_names=None, textures=None, bindings_wgsl=None))]
    pub fn new(
        py: Python<'_>,
        base: &Bound<'_, PyAny>,
        fragment_shader: Option<String>,
        vertex_shader: Option<String>,
        data: Option<Vec<f32>>,
        shader_defs: u32,
        shader_def_names: Option<Vec<String>>,
        textures: Option<Py<PyList>>,
        bindings_wgsl: Option<String>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let shader_def_names = shader_def_names.unwrap_or_default();
        validate_shader_defs(shader_defs, &shader_def_names)?;
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
        let mut tex_handles: [Option<Handle<Image>>; MAX_TEXTURE_SLOTS] = Default::default();

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
                    let typed: Handle<Image> = (&py_handle).try_into()?;
                    tex_handles[i] = Some(typed);
                }
            }
        }

        let mut py_mat = base.extract::<PyRefMut<'_, PyStandardMaterial>>()?;
        let std_mat: StandardMaterial = py_mat.take()?;

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
                shader_def_names,
                logical_type_id: None,
                bindings_wgsl,
            },
        };
        Ok(Self::from_owned(material).into())
    }

    #[getter]
    fn _logical_type_id(&self) -> PyResult<Option<u64>> {
        Ok(self.as_ref()?.extension.logical_type_id)
    }

    fn _set_logical_type_id(&mut self, logical_type_id: u64) -> PyResult<()> {
        self.as_mut()?.extension.logical_type_id = Some(logical_type_id);
        Ok(())
    }

    fn get_shader_defs(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.extension.shader_defs)
    }

    fn set_shader_defs(&mut self, defs: u32) -> PyResult<()> {
        {
            let material = self.as_ref()?;
            validate_shader_defs(defs, &material.extension.shader_def_names)?;
        }
        self.as_mut()?.extension.shader_defs = defs;
        Ok(())
    }

    fn set_texture(&mut self, slot: isize, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        let slot = checked_index(slot, "texture slot")?;
        if slot >= MAX_TEXTURE_SLOTS {
            return Err(PyIndexError::new_err(format!(
                "texture slot {} out of range (max {})",
                slot,
                MAX_TEXTURE_SLOTS - 1
            )));
        }
        let py_handle = extract_handle_from_any(handle)?;
        let typed: Handle<Image> = (&py_handle).try_into()?;
        let mut mat = self.as_mut()?;
        match slot {
            0 => mat.extension.texture_0 = Some(typed),
            1 => mat.extension.texture_1 = Some(typed),
            2 => mat.extension.texture_2 = Some(typed),
            3 => mat.extension.texture_3 = Some(typed),
            _ => unreachable!("slot was range-checked"),
        }
        Ok(())
    }

    fn clear_texture(&mut self, slot: isize) -> PyResult<()> {
        let slot = checked_index(slot, "texture slot")?;
        if slot >= MAX_TEXTURE_SLOTS {
            return Err(PyIndexError::new_err(format!(
                "texture slot {} out of range (max {})",
                slot,
                MAX_TEXTURE_SLOTS - 1
            )));
        }
        let mut mat = self.as_mut()?;
        match slot {
            0 => mat.extension.texture_0 = None,
            1 => mat.extension.texture_1 = None,
            2 => mat.extension.texture_2 = None,
            3 => mat.extension.texture_3 = None,
            _ => unreachable!("slot was range-checked"),
        }
        Ok(())
    }

    fn set_data_float(&mut self, index: isize, value: f32) -> PyResult<()> {
        let index = checked_index(index, "data index")?;
        if index >= 256 {
            return Err(PyIndexError::new_err(format!(
                "float index {} out of range (max 255)",
                index
            )));
        }
        let mut mat = self.as_mut()?;
        mat.extension.params.data[index / 4][index % 4] = value;
        Ok(())
    }

    fn get_data_float(&self, index: isize) -> PyResult<f32> {
        let index = checked_index(index, "data index")?;
        if index >= 256 {
            return Err(PyIndexError::new_err(format!(
                "float index {} out of range (max 255)",
                index
            )));
        }
        let mat = self.as_ref()?;
        Ok(mat.extension.params.data[index / 4][index % 4])
    }

    fn set_data_floats(&mut self, start: isize, values: &Bound<'_, PyAny>) -> PyResult<()> {
        let start = checked_index(start, "start")?;
        let values = values.extract::<Vec<f32>>().map_err(|_| {
            let actual = values
                .get_type()
                .name()
                .map_or_else(|_| "object".into(), |name| name.to_string());
            PyTypeError::new_err(expected_float_sequence(actual))
        })?;
        let end = start.saturating_add(values.len());
        if end > 256 {
            return Err(PyIndexError::new_err(format!(
                "float range {}..{} exceeds buffer (max 256)",
                start, end
            )));
        }
        let mut mat = self.as_mut()?;
        for (i, &val) in values.iter().enumerate() {
            let idx = start + i;
            mat.extension.params.data[idx / 4][idx % 4] = val;
        }
        Ok(())
    }

    fn get_data_floats(&self, start: isize, count: isize) -> PyResult<Vec<f32>> {
        let start = checked_index(start, "start")?;
        let count = checked_index(count, "count")?;
        let end = start.saturating_add(count);
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

#[pyplugin(MaterialPlugin::<ShaderMaterial>)]
#[pyclass(name = "ShaderMaterialPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyShaderMaterialPlugin;

#[pymethods]
impl PyShaderMaterialPlugin {
    #[new]
    #[pyo3(signature = ())]
    pub fn new() -> PyClassInitializer<Self> {
        (PyShaderMaterialPlugin, PyPlugin).into()
    }
}

impl PluginBuild for PyShaderMaterialPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        crate::shader_material::clear_shader_registries();
        app.add_plugins(MaterialPlugin::<ShaderMaterial>::default());
        app.add_systems(Last, crate::shader_material::sync_shader_handles);
        Ok(())
    }
}

#[pycomponent(MeshMaterial3d<ShaderMaterial>, bridge)]
#[pyclass(name = "MeshMaterial3dShader", extends = PyComponent, eq, skip_from_py_object)]
#[derive(Debug, PartialEq)]
pub struct PyMeshMaterial3dShader {
    pub(crate) storage: ComponentStorage<MeshMaterial3d<ShaderMaterial>>,
    logical_type_id: Option<LogicalTypeId>,
}

fn extract_shader_material_handle(
    handle: &Bound<'_, PyAny>,
) -> PyResult<(Handle<ShaderMaterial>, Option<LogicalTypeId>)> {
    let handle = extract_handle_from_any(handle)?;

    ensure_asset_type::<ShaderMaterial>(&handle)?;

    let logical_type_id = handle.logical_type_id();
    Ok(((&handle).try_into()?, logical_type_id))
}

#[pymethods]
impl PyMeshMaterial3dShader {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let (handle, logical_type_id) = extract_shader_material_handle(handle)?;
        let (mut component, base) = Self::from_owned(MeshMaterial3d(handle));
        component.logical_type_id = logical_type_id;
        Ok((component, base).into())
    }

    #[getter]
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(&self.as_ref()?.0).with_logical_type_id(self.logical_type_id))
    }

    #[setter]
    pub fn set_handle(&mut self, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        let (handle, logical_type_id) = extract_shader_material_handle(handle)?;
        if let Some(expected) = self.logical_type_id
            && logical_type_id != Some(expected)
        {
            return Err(PyTypeError::new_err(
                "Material type mismatch: the handle is untyped or belongs to a different @material class",
            ));
        }
        self.as_mut()?.0 = handle;
        Ok(())
    }

    #[getter]
    fn _logical_type_id(&self) -> Option<u64> {
        self.logical_type_id.map(LogicalTypeId::get)
    }

    fn _with_logical_type_id(
        slf: Py<Self>,
        py: Python<'_>,
        logical_type_id: u64,
    ) -> PyResult<Py<Self>> {
        let logical_type_id = LogicalTypeId::new(logical_type_id);
        if slf.borrow(py).logical_type_id != Some(logical_type_id) {
            return Err(PyTypeError::new_err(
                "Material type mismatch: the handle is untyped or belongs to a different @material class",
            ));
        }
        Ok(slf)
    }

    fn _materialize_logical_type_id(
        slf: Py<Self>,
        py: Python<'_>,
        logical_type_id: u64,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).logical_type_id = Some(LogicalTypeId::new(logical_type_id));
        Ok(slf)
    }
}
