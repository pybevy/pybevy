use std::mem::size_of;

use bevy::{
    app::App,
    asset::Handle,
    color::LinearRgba,
    image::Image,
    math::{Vec2, Vec3, Vec4},
    pbr::{MeshMaterial3d, StandardMaterial},
};
use pybevy_color::linear_rgba::PyLinearRgba;
use pybevy_core::{
    AssetInputConverter, AssetStorage, ComponentStorage, FromBorrowedStorage, HandleArg,
    LogicalTypeId, NativeAsset, PluginBuild, PyComponent, PyHandle, PyMaterial, PyPlugin,
    ValueStorage,
    asset_path::parse_asset_path,
    ensure_asset_type, extract_handle_from_any,
    public_error::{
        shader_data_too_long, shader_texture_slot_out_of_range, unsupported_packed_uniform_type,
    },
};
use pybevy_macros::{pyasset, pycomponent, pyplugin};
use pybevy_math::{vec2::PyVec2, vec3::PyVec3, vec4::PyVec4};
use pybevy_shader_types::ShaderMaterialPlugin;
use pyo3::{
    PyTypeInfo,
    exceptions::{PyIndexError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyList, PyType},
};

use crate::{
    material_access::{self, validate_shader_defs},
    shader_material::{
        MAX_TEXTURE_SLOTS, ShaderMaterial, ShaderMaterialExtension, ShaderParams,
        packed_uniform::UniformData, validate_material_shader_path,
    },
    standard_material::PyStandardMaterial,
};

const MAX_DATA_FLOATS: usize = size_of::<UniformData>() / size_of::<f32>();

#[pyasset(
    ShaderMaterial,
    bridge,
    not_loadable,
    input_converter,
    material,
    missing_resource_hint = "Add ShaderMaterialPlugin() to your app to create this collection."
)]
#[pyclass(name = "ShaderMaterial", module = "pybevy.pbr", extends = PyMaterial, skip_from_py_object)]
#[derive(Debug)]
pub struct PyShaderMaterial {
    pub(crate) storage: AssetStorage<ShaderMaterial>,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_shader_material(
    py: Python<'_>,
    base: &Bound<'_, PyAny>,
    fragment_shader: Option<String>,
    vertex_shader: Option<String>,
    data: Option<Vec<f32>>,
    shader_defs: u32,
    shader_def_names: Option<Vec<String>>,
    textures: Option<Py<PyList>>,
    bindings_wgsl: Option<String>,
    prepass_vertex_shader: Option<String>,
    prepass_fragment_shader: Option<String>,
    deferred_vertex_shader: Option<String>,
    deferred_fragment_shader: Option<String>,
) -> PyResult<ShaderMaterial> {
    let shader_def_names = shader_def_names.unwrap_or_default();
    validate_shader_defs(shader_defs, &shader_def_names)?;
    for path in [
        &fragment_shader,
        &vertex_shader,
        &prepass_vertex_shader,
        &prepass_fragment_shader,
        &deferred_vertex_shader,
        &deferred_fragment_shader,
    ]
    .into_iter()
    .flatten()
    {
        parse_asset_path(path)?;
        validate_material_shader_path(path).map_err(PyValueError::new_err)?;
    }
    let mut params = ShaderParams::default();

    if let Some(data) = data {
        if data.len() > MAX_DATA_FLOATS {
            return Err(PyValueError::new_err(shader_data_too_long(
                data.len(),
                MAX_DATA_FLOATS,
            )));
        }
        for (i, &val) in data.iter().enumerate() {
            let vec_idx = i / 4;
            let comp_idx = i % 4;
            params.data.words[vec_idx][comp_idx] = val.to_bits();
        }
    }

    // Extract texture handles from Python list [handle_or_None, ...]
    let mut tex_handles: [Option<Handle<Image>>; MAX_TEXTURE_SLOTS] = Default::default();

    if let Some(tex_list) = textures {
        let list = tex_list.bind(py);
        for (i, item) in list.iter().enumerate() {
            if i >= MAX_TEXTURE_SLOTS {
                return Err(PyIndexError::new_err(shader_texture_slot_out_of_range(
                    i,
                    MAX_TEXTURE_SLOTS - 1,
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
            prepass_vertex_shader_path: prepass_vertex_shader,
            prepass_fragment_shader_path: prepass_fragment_shader,
            deferred_vertex_shader_path: deferred_vertex_shader,
            deferred_fragment_shader_path: deferred_fragment_shader,
            shader_defs,
            shader_def_names,
            logical_type_id: None,
            bindings_wgsl,
        },
    };
    Ok(material)
}

#[pymethods]
impl PyShaderMaterial {
    #[getter]
    pub fn base(&self, py: Python<'_>) -> PyResult<Py<PyStandardMaterial>> {
        let storage = self
            .storage
            .borrow_asset_field(|material| &material.base, |material| &mut material.base)?;
        Py::new(py, PyStandardMaterial::from_borrowed(storage))
    }

    #[setter]
    pub fn set_base(&mut self, base: PyRef<'_, PyStandardMaterial>) -> PyResult<()> {
        let base = base.storage.as_ref()?.clone();
        self.as_mut()?.base = base;
        Ok(())
    }

    #[staticmethod]
    fn _read_only_base(
        py: Python<'_>,
        base: PyRef<'_, PyStandardMaterial>,
    ) -> PyResult<Py<PyStandardMaterial>> {
        let storage = AssetStorage::read_only_snapshot(base.storage.as_ref()?.clone());
        Py::new(py, PyStandardMaterial::from_borrowed(storage))
    }

    #[staticmethod]
    fn _validate_shader_def_names(names: Vec<String>) -> PyResult<()> {
        pybevy_shader_types::shader_def_registry::validate_names(&names)
            .map_err(PyTypeError::new_err)
    }

    #[staticmethod]
    fn _validate_bindings(source: &str) -> PyResult<()> {
        let source = source
            .lines()
            .filter(|line| !line.trim().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        pybevy_shader_types::storage_element::validate_bindings(&source)
            .map_err(PyTypeError::new_err)
    }
    #[classattr]
    fn __pybevy_component_type__(py: Python<'_>) -> Bound<'_, PyType> {
        PyMeshMaterial3dShader::type_object(py)
    }

    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (base, fragment_shader=None, vertex_shader=None, data=None, shader_defs=0, shader_def_names=None, textures=None, bindings_wgsl=None, *, prepass_vertex_shader=None, prepass_fragment_shader=None, deferred_vertex_shader=None, deferred_fragment_shader=None))]
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
        prepass_vertex_shader: Option<String>,
        prepass_fragment_shader: Option<String>,
        deferred_vertex_shader: Option<String>,
        deferred_fragment_shader: Option<String>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(build_shader_material(
            py,
            base,
            fragment_shader,
            vertex_shader,
            data,
            shader_defs,
            shader_def_names,
            textures,
            bindings_wgsl,
            prepass_vertex_shader,
            prepass_fragment_shader,
            deferred_vertex_shader,
            deferred_fragment_shader,
        )?))
    }

    #[getter]
    fn _logical_type_id(&self) -> PyResult<Option<u64>> {
        material_access::_logical_type_id(&self.storage)
    }

    fn _set_logical_type_id(&mut self, logical_type_id: u64) -> PyResult<()> {
        material_access::_set_logical_type_id(&mut self.storage, logical_type_id)
    }

    fn get_shader_defs(&self) -> PyResult<u32> {
        material_access::get_shader_defs(&self.storage)
    }

    fn set_shader_defs(&mut self, defs: u32) -> PyResult<()> {
        material_access::set_shader_defs(&mut self.storage, defs)
    }

    fn get_texture(&self, slot: isize) -> PyResult<Option<PyHandle>> {
        material_access::get_texture(&self.storage, slot)
    }

    fn set_texture(&mut self, slot: isize, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        material_access::set_texture(&mut self.storage, slot, handle)
    }

    fn clear_texture(&mut self, slot: isize) -> PyResult<()> {
        material_access::clear_texture(&mut self.storage, slot)
    }

    fn _get_data_int(&self, index: isize) -> PyResult<i32> {
        material_access::get_data_int(&self.storage, index)
    }
    fn _set_data_int(&mut self, index: isize, value: i32) -> PyResult<()> {
        material_access::set_data_int(&mut self.storage, index, value)
    }

    fn set_data_float(&mut self, index: isize, value: f32) -> PyResult<()> {
        material_access::set_data_float(&mut self.storage, index, value)
    }

    fn get_data_float(&self, index: isize) -> PyResult<f32> {
        material_access::get_data_float(&self.storage, index)
    }

    fn set_data_floats(&mut self, start: isize, values: &Bound<'_, PyAny>) -> PyResult<()> {
        material_access::set_data_floats(&mut self.storage, start, values)
    }

    fn _uniform_field(
        &self,
        py: Python<'_>,
        float_index: isize,
        value_type: &Bound<'_, PyType>,
    ) -> PyResult<Py<PyAny>> {
        material_access::borrow_field(&self.storage, py, float_index, value_type)
    }

    #[staticmethod]
    fn _read_only_uniform(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(value) = value.extract::<PyRef<'_, PyVec2>>() {
            let raw: Vec2 = (&*value).try_into()?;
            let snapshot = PyVec2::from_borrowed(ValueStorage::read_only_snapshot(raw));
            return Ok(Py::new(py, snapshot)?.into_any());
        }
        if let Ok(value) = value.extract::<PyRef<'_, PyVec3>>() {
            let raw: Vec3 = (&*value).try_into()?;
            let snapshot = PyVec3::from_borrowed(ValueStorage::read_only_snapshot(raw));
            return Ok(Py::new(py, snapshot)?.into_any());
        }
        if let Ok(value) = value.extract::<PyRef<'_, PyVec4>>() {
            let raw: Vec4 = (&*value).try_into()?;
            let snapshot = PyVec4::from_borrowed(ValueStorage::read_only_snapshot(raw));
            return Ok(Py::new(py, snapshot)?.into_any());
        }
        if let Ok(value) = value.extract::<PyRef<'_, PyLinearRgba>>() {
            let raw: LinearRgba = (&*value).try_into()?;
            let snapshot = PyLinearRgba::from_borrowed(ValueStorage::read_only_snapshot(raw));
            return Ok(Py::new(py, snapshot)?.into_any());
        }
        Err(PyTypeError::new_err(unsupported_packed_uniform_type(
            value.get_type().name()?,
        )))
    }

    fn get_data_floats(&self, start: isize, count: isize) -> PyResult<Vec<f32>> {
        material_access::get_data_floats(&self.storage, start, count)
    }
}

#[pyplugin(ShaderMaterialPlugin)]
#[pyclass(name = "ShaderMaterialPlugin", module = "pybevy.pbr", extends = PyPlugin, frozen, skip_from_py_object)]
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
        if !app.is_plugin_added::<ShaderMaterialPlugin>() {
            app.add_plugins(ShaderMaterialPlugin);
        }
        Ok(())
    }
}

#[pycomponent(MeshMaterial3d<ShaderMaterial>, bridge)]
#[pyclass(name = "MeshMaterial3dShader", module = "pybevy.pbr", extends = PyComponent, eq, skip_from_py_object)]
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
    #[pyo3(signature = (handle = HandleArg::default()))]
    pub fn new(handle: HandleArg) -> PyResult<PyClassInitializer<Self>> {
        let logical_type_id = match &handle {
            HandleArg::Supplied(handle) => handle.logical_type_id(),
            HandleArg::Omitted => None,
        };
        let (mut component, base) = Self::from_owned(MeshMaterial3d(handle.into_handle()?));
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
