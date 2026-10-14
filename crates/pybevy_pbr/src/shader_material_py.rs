use std::mem::size_of;

use bevy::{
    app::{App, Last},
    asset::Handle,
    color::LinearRgba,
    image::Image,
    math::{Vec2, Vec3, Vec4},
    pbr::{MaterialPlugin, MeshMaterial3d, StandardMaterial},
};
use pybevy_color::linear_rgba::PyLinearRgba;
use pybevy_core::{
    AssetInputConverter, AssetStorage, ComponentStorage, FieldStorage, FromBorrowedStorage,
    HandleArg, LogicalTypeId, NativeAsset, PluginBuild, PyComponent, PyHandle, PyMaterial,
    PyPlugin, ValueStorage,
    asset_path::parse_asset_path,
    ensure_asset_type, extract_handle_from_any,
    public_error::{
        SHADER_DEFS_WITHOUT_NAMES, expected_float_sequence, non_negative_argument,
        packed_uniform_misaligned, packed_uniform_out_of_range, shader_data_too_long,
        shader_float_index_out_of_range, shader_float_range_out_of_range,
        shader_texture_slot_out_of_range, too_many_shader_def_names,
        unsupported_packed_uniform_type,
    },
};
use pybevy_macros::{pyasset, pycomponent, pyplugin};
use pybevy_math::{vec2::PyVec2, vec3::PyVec3, vec4::PyVec4};
use pyo3::{
    exceptions::{PyIndexError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyList, PyType},
};

use crate::{
    shader_material::{
        MAX_TEXTURE_SLOTS, ShaderMaterial, ShaderMaterialExtension, ShaderParams,
        packed_uniform::{
            PackedUniform, UniformData, UniformIndexError, check_uniform_index, uniform_mut,
            uniform_ref,
        },
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

fn checked_index(value: isize, name: &str) -> PyResult<usize> {
    usize::try_from(value).map_err(|_| PyIndexError::new_err(non_negative_argument(name, value)))
}

fn checked_texture_slot(slot: isize) -> PyResult<usize> {
    let slot = checked_index(slot, "texture slot")?;
    if slot >= MAX_TEXTURE_SLOTS {
        return Err(PyIndexError::new_err(shader_texture_slot_out_of_range(
            slot,
            MAX_TEXTURE_SLOTS - 1,
        )));
    }
    Ok(slot)
}

fn check_packed_uniform<F: PackedUniform>(float_index: usize) -> PyResult<()> {
    check_uniform_index::<F>(float_index).map_err(|error| match error {
        UniformIndexError::Misaligned => PyValueError::new_err(packed_uniform_misaligned(
            F::NAME,
            float_index,
            F::ALIGNMENT_FLOATS,
        )),
        UniformIndexError::OutOfRange => {
            PyIndexError::new_err(packed_uniform_out_of_range(F::NAME, float_index, F::FLOATS))
        }
    })
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

impl PyShaderMaterial {
    fn replace_texture(&mut self, slot: usize, handle: Option<Handle<Image>>) -> PyResult<()> {
        let mut material = self.as_mut()?;
        if let Some(texture) = material.extension.texture_slot_mut(slot) {
            *texture = handle;
        }
        Ok(())
    }

    fn uniform_field<F, W>(&self, float_index: usize) -> PyResult<W>
    where
        F: PackedUniform + 'static,
        W: FromBorrowedStorage<ValueStorage<F>>,
    {
        check_packed_uniform::<F>(float_index)?;
        let data: FieldStorage<UniformData> = self.storage.borrow_field(
            |material| &material.extension.params.data,
            |material| &mut material.extension.params.data,
        )?;
        Ok(data.borrow_resolved_index_as::<F, ValueStorage<F>, W>(
            float_index,
            uniform_ref::<F>,
            uniform_mut::<F>,
        )?)
    }
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
        if let Some(path) = &fragment_shader {
            parse_asset_path(path)?;
        }
        if let Some(path) = &vertex_shader {
            parse_asset_path(path)?;
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
                params.data[vec_idx][comp_idx] = val;
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
                shader_defs,
                shader_def_names,
                logical_type_id: None,
                bindings_wgsl,
            },
        };
        Ok(Self::from_owned(material))
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

    fn get_texture(&self, slot: isize) -> PyResult<Option<PyHandle>> {
        let slot = checked_texture_slot(slot)?;
        let material = self.as_ref()?;
        Ok(material
            .extension
            .texture_slot(slot)
            .and_then(Option::as_ref)
            .map(PyHandle::from))
    }

    fn set_texture(&mut self, slot: isize, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        let slot = checked_texture_slot(slot)?;
        let py_handle = extract_handle_from_any(handle)?;
        let typed: Handle<Image> = (&py_handle).try_into()?;
        self.replace_texture(slot, Some(typed))
    }

    fn clear_texture(&mut self, slot: isize) -> PyResult<()> {
        let slot = checked_texture_slot(slot)?;
        self.replace_texture(slot, None)
    }

    fn set_data_float(&mut self, index: isize, value: f32) -> PyResult<()> {
        let index = checked_index(index, "data index")?;
        if index >= MAX_DATA_FLOATS {
            return Err(PyIndexError::new_err(shader_float_index_out_of_range(
                index,
                MAX_DATA_FLOATS - 1,
            )));
        }
        let mut mat = self.as_mut()?;
        mat.extension.params.data[index / 4][index % 4] = value;
        Ok(())
    }

    fn get_data_float(&self, index: isize) -> PyResult<f32> {
        let index = checked_index(index, "data index")?;
        if index >= MAX_DATA_FLOATS {
            return Err(PyIndexError::new_err(shader_float_index_out_of_range(
                index,
                MAX_DATA_FLOATS - 1,
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
        if end > MAX_DATA_FLOATS {
            return Err(PyIndexError::new_err(shader_float_range_out_of_range(
                start,
                end,
                MAX_DATA_FLOATS,
            )));
        }
        let mut mat = self.as_mut()?;
        for (i, &val) in values.iter().enumerate() {
            let idx = start + i;
            mat.extension.params.data[idx / 4][idx % 4] = val;
        }
        Ok(())
    }

    fn _uniform_field(
        &self,
        py: Python<'_>,
        float_index: isize,
        value_type: &Bound<'_, PyType>,
    ) -> PyResult<Py<PyAny>> {
        let index = checked_index(float_index, "float index")?;
        if value_type.is(py.get_type::<PyVec2>()) {
            let value: PyVec2 = self.uniform_field::<Vec2, _>(index)?;
            return Ok(Py::new(py, value)?.into_any());
        }
        if value_type.is(py.get_type::<PyVec3>()) {
            let value: PyVec3 = self.uniform_field::<Vec3, _>(index)?;
            return Ok(Py::new(py, value)?.into_any());
        }
        if value_type.is(py.get_type::<PyVec4>()) {
            let value: PyVec4 = self.uniform_field::<Vec4, _>(index)?;
            return Ok(Py::new(py, value)?.into_any());
        }
        if value_type.is(py.get_type::<PyLinearRgba>()) {
            let value: PyLinearRgba = self.uniform_field::<LinearRgba, _>(index)?;
            return Ok(Py::new(py, value)?.into_any());
        }
        Err(PyTypeError::new_err(unsupported_packed_uniform_type(
            value_type.name()?,
        )))
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
        let start = checked_index(start, "start")?;
        let count = checked_index(count, "count")?;
        let end = start.saturating_add(count);
        if end > MAX_DATA_FLOATS {
            return Err(PyIndexError::new_err(shader_float_range_out_of_range(
                start,
                end,
                MAX_DATA_FLOATS,
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
        crate::shader_material::clear_shader_registries();
        app.add_plugins(MaterialPlugin::<ShaderMaterial>::default());
        app.add_systems(Last, crate::shader_material::sync_shader_handles);
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
