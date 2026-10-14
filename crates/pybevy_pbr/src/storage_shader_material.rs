use bevy::{
    asset::Handle,
    image::{Image, ImageSamplerDescriptor},
    pbr::MeshMaterial3d,
    render::storage::ShaderBuffer,
};
use pybevy_core::{
    AssetInputConverter, AssetStorage, ComponentStorage, FieldStorage, HandleArg, LogicalTypeId,
    PyComponent, PyHandle, PyMaterial, ensure_asset_type, extract_handle_from_any,
    public_error::shader_texture_slot_out_of_range,
};
use pybevy_image::sampler_descriptor::PyImageSamplerDescriptor;
use pybevy_macros::{pyasset, pycomponent};
use pybevy_shader_types::{
    MAX_TEXTURE_SLOTS,
    sampler_binding::{MaterialSamplerBinding, MaterialSamplerKind},
    storage_element::StorageElement,
    storage_material::StorageBinding,
    texture_binding::TextureBindingLayout,
};
use pyo3::{
    PyTypeInfo,
    exceptions::{PyIndexError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyList, PyType},
};

use crate::{
    material_access,
    shader_material::{StorageShaderMaterial, StorageShaderMaterialExtension},
    shader_material_py::build_shader_material,
    standard_material::PyStandardMaterial,
};

type TextureLayoutInput = (String, String, bool, Option<String>);

fn extract_storage_handle(
    handle: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<Handle<ShaderBuffer>>> {
    handle
        .filter(|value| !value.is_none())
        .map(|value| {
            let handle = extract_handle_from_any(value)?;
            (&handle).try_into()
        })
        .transpose()
}

#[pyasset(
    StorageShaderMaterial,
    bridge,
    not_loadable,
    input_converter,
    material,
    missing_resource_hint = "Add ShaderMaterialPlugin() to your app to create this collection."
)]
#[pyclass(name = "StorageShaderMaterial", module = "pybevy.pbr", extends = PyMaterial, skip_from_py_object)]
#[derive(Debug)]
pub struct PyStorageShaderMaterial {
    pub(crate) storage: AssetStorage<StorageShaderMaterial>,
}

impl AssetInputConverter for PyStorageShaderMaterial {
    fn try_convert_input<'py>(
        asset: &Bound<'py, PyAny>,
        _: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        if let Ok(converter) = asset.getattr("to_shader_material") {
            return Ok(Some(converter.call0()?));
        }
        Ok(None)
    }
}

type ShaderStructLayoutTuple = (u32, u32, u32, Vec<(String, u32, u32, u32)>);

#[pymethods]
impl PyStorageShaderMaterial {
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
    fn _copy_sampler_descriptor(
        value: &PyImageSamplerDescriptor,
    ) -> PyResult<PyImageSamplerDescriptor> {
        Ok(ImageSamplerDescriptor::try_from(value)?.into())
    }
    #[staticmethod]
    fn _validate_storage_handle(value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        extract_storage_handle(value).map(|_| ())
    }
    #[staticmethod]
    fn _storage_element_layout(element: &str, declarations: &str) -> PyResult<(u32, u32)> {
        let element =
            StorageElement::parse(element, declarations).map_err(PyValueError::new_err)?;
        Ok((element.stride, element.alignment))
    }
    #[staticmethod]
    fn _storage_struct_layout(
        element: &str,
        declarations: &str,
    ) -> PyResult<ShaderStructLayoutTuple> {
        let layout =
            StorageElement::struct_layout(element, declarations).map_err(PyValueError::new_err)?;
        Ok((
            layout.size,
            layout.stride,
            layout.alignment,
            layout
                .fields
                .into_iter()
                .map(|field| (field.name, field.offset, field.size, field.alignment))
                .collect(),
        ))
    }
    #[staticmethod]
    fn _validate_texture_handle(value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        if let Some(value) = value.filter(|value| !value.is_none()) {
            let handle = extract_handle_from_any(value)?;
            let _: Handle<Image> = (&handle).try_into()?;
        }
        Ok(())
    }
    #[staticmethod]
    fn _texture_layout(
        dimension: &str,
        sample_type: &str,
        filterable: bool,
        sampler: Option<&str>,
    ) -> PyResult<(String, String)> {
        Ok(TextureBindingLayout::parse(
            dimension,
            sample_type,
            filterable,
            sampler.unwrap_or("none"),
        )
        .map_err(PyValueError::new_err)?
        .wgsl_types())
    }
    #[classattr]
    fn __pybevy_component_type__(py: Python<'_>) -> Bound<'_, PyType> {
        PyMeshMaterial3dStorageShader::type_object(py)
    }

    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (base, fragment_shader=None, vertex_shader=None, data=None, shader_defs=0, shader_def_names=None, textures=None, bindings_wgsl=None, *, storage_buffer=None, storage_buffers=None, storage_layouts=None, texture_layouts=None, samplers=None, prepass_vertex_shader=None, prepass_fragment_shader=None, deferred_vertex_shader=None, deferred_fragment_shader=None))]
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
        storage_buffer: Option<&Bound<'_, PyAny>>,
        storage_buffers: Option<&Bound<'_, PyList>>,
        storage_layouts: Option<Vec<(String, String)>>,
        texture_layouts: Option<Vec<Option<TextureLayoutInput>>>,
        samplers: Option<Vec<(String, PyImageSamplerDescriptor)>>,
        prepass_vertex_shader: Option<String>,
        prepass_fragment_shader: Option<String>,
        deferred_vertex_shader: Option<String>,
        deferred_fragment_shader: Option<String>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if storage_buffers.is_some() && storage_buffer.is_some_and(|value| !value.is_none()) {
            return Err(PyTypeError::new_err(
                "Use storage_buffer or storage_buffers, not both",
            ));
        }
        let texture_layouts = texture_layouts.unwrap_or_default();
        if texture_layouts.len() > MAX_TEXTURE_SLOTS {
            return Err(PyIndexError::new_err(shader_texture_slot_out_of_range(
                MAX_TEXTURE_SLOTS,
                MAX_TEXTURE_SLOTS - 1,
            )));
        }
        let texture_layouts = texture_layouts
            .into_iter()
            .map(|value| {
                value
                    .map(|(dimension, sample, filterable, sampler)| {
                        TextureBindingLayout::parse(
                            &dimension,
                            &sample,
                            filterable,
                            sampler.as_deref().unwrap_or("none"),
                        )
                        .map_err(PyValueError::new_err)
                    })
                    .transpose()
            })
            .collect::<PyResult<Vec<_>>>()?;
        let samplers = samplers
            .unwrap_or_default()
            .into_iter()
            .map(|(kind, descriptor)| {
                Ok(MaterialSamplerBinding {
                    kind: MaterialSamplerKind::parse(&kind).map_err(PyValueError::new_err)?,
                    descriptor: ImageSamplerDescriptor::try_from(&descriptor)?,
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let handles = match storage_buffers {
            Some(values) => values
                .iter()
                .map(|value| extract_storage_handle(Some(&value)))
                .collect::<PyResult<Vec<_>>>()?,
            None => vec![extract_storage_handle(storage_buffer)?],
        };
        let layouts = match storage_layouts {
            Some(values) => {
                StorageElement::parse_layouts(&values).map_err(PyValueError::new_err)?
            }
            None => vec![StorageElement::default(); handles.len()],
        };
        if layouts.len() != handles.len() {
            return Err(PyValueError::new_err(
                "storage_layouts must match storage_buffers length",
            ));
        }
        let buffers = handles
            .into_iter()
            .zip(layouts)
            .map(|(handle, element)| StorageBinding { handle, element })
            .collect();
        let material = build_shader_material(
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
        )?;
        Ok(Self::from_owned(StorageShaderMaterial {
            base: material.base,
            extension: StorageShaderMaterialExtension {
                inner: material.extension,
                buffers,
                texture_layouts,
                samplers,
            },
        }))
    }

    fn get_sampler(&self, slot: isize) -> PyResult<PyImageSamplerDescriptor> {
        // A negative slot is out of range, like any slot past the end.
        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
        let values: FieldStorage<Vec<MaterialSamplerBinding>> = self.storage.borrow_field(
            |material| &material.extension.samplers,
            |material| &mut material.extension.samplers,
        )?;
        Ok(values.borrow_resolved_index_as::<ImageSamplerDescriptor, FieldStorage<ImageSamplerDescriptor>, PyImageSamplerDescriptor>(
            slot, |values, index| values.get(index).map(|s| &s.descriptor),
            |values, index| values.get_mut(index).map(|s| &mut s.descriptor),
        )?)
    }
    fn set_sampler(&mut self, slot: isize, descriptor: &PyImageSamplerDescriptor) -> PyResult<()> {
        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
        let descriptor = ImageSamplerDescriptor::try_from(descriptor)?;
        if slot >= self.as_ref()?.extension.samplers.len() {
            return Err(PyIndexError::new_err("Sampler slot out of range"));
        }
        self.as_mut()?.extension.samplers[slot].descriptor = descriptor;
        Ok(())
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
    fn get_data_floats(&self, start: isize, count: isize) -> PyResult<Vec<f32>> {
        material_access::get_data_floats(&self.storage, start, count)
    }

    #[getter]
    fn storage_buffer(&self) -> PyResult<Option<PyHandle>> {
        self.buffer_at(0)
    }
    #[setter(storage_buffer)]
    fn set_first_storage_buffer(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.set_buffer_at(0, value)
    }
    #[pyo3(name = "get_storage_buffer_at")]
    fn buffer_at(&self, slot: isize) -> PyResult<Option<PyHandle>> {
        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
        let material = self.as_ref()?;
        let binding = material
            .extension
            .buffers
            .get(slot)
            .ok_or_else(|| PyIndexError::new_err("Storage buffer slot out of range"))?;
        Ok(binding.handle.as_ref().map(PyHandle::from))
    }
    # [pyo3(name = "set_storage_buffer_at", signature = (slot, value=None))]
    fn set_buffer_at(&mut self, slot: isize, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
        let handle = extract_storage_handle(value)?;
        if slot >= self.as_ref()?.extension.buffers.len() {
            return Err(PyIndexError::new_err("Storage buffer slot out of range"));
        }
        self.as_mut()?.extension.buffers[slot].handle = handle;
        Ok(())
    }
}

#[pycomponent(MeshMaterial3d<StorageShaderMaterial>, bridge)]
#[pyclass(name = "MeshMaterial3dStorageShader", module = "pybevy.pbr", extends = PyComponent, eq, skip_from_py_object)]
#[derive(Debug, PartialEq)]
pub struct PyMeshMaterial3dStorageShader {
    pub(crate) storage: ComponentStorage<MeshMaterial3d<StorageShaderMaterial>>,
    logical_type_id: Option<LogicalTypeId>,
}

fn extract_storage_material_handle(
    handle: &Bound<'_, PyAny>,
) -> PyResult<(Handle<StorageShaderMaterial>, Option<LogicalTypeId>)> {
    let handle = extract_handle_from_any(handle)?;

    ensure_asset_type::<StorageShaderMaterial>(&handle)?;

    let logical_type_id = handle.logical_type_id();
    Ok(((&handle).try_into()?, logical_type_id))
}

#[pymethods]
impl PyMeshMaterial3dStorageShader {
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
        let (handle, logical_type_id) = extract_storage_material_handle(handle)?;
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
