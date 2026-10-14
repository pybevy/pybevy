use std::mem::size_of;

use bevy::{
    asset::Handle,
    color::LinearRgba,
    image::Image,
    math::{Vec2, Vec3, Vec4},
};
use pybevy_color::linear_rgba::PyLinearRgba;
use pybevy_core::{
    AssetStorage, FieldStorage, FromBorrowedStorage, PyHandle, ValueStorage,
    extract_handle_from_any,
    public_error::{
        SHADER_DEFS_WITHOUT_NAMES, expected_float_sequence, non_negative_argument,
        packed_uniform_misaligned, packed_uniform_out_of_range, shader_float_index_out_of_range,
        shader_float_range_out_of_range, shader_texture_slot_out_of_range,
        too_many_shader_def_names, unsupported_packed_uniform_type,
    },
};
use pybevy_math::{vec2::PyVec2, vec3::PyVec3, vec4::PyVec4};
use pyo3::{
    exceptions::{PyIndexError, PyTypeError, PyValueError},
    prelude::*,
    types::PyType,
};

use crate::shader_material::{
    MAX_TEXTURE_SLOTS, ShaderMaterialData,
    packed_uniform::{
        PackedUniform, UniformData, UniformIndexError, check_uniform_index, uniform_mut,
        uniform_ref,
    },
};

const MAX_DATA_FLOATS: usize = size_of::<UniformData>() / size_of::<f32>();

pub(crate) fn checked_index(value: isize, name: &str) -> PyResult<usize> {
    usize::try_from(value).map_err(|_| PyIndexError::new_err(non_negative_argument(name, value)))
}

pub(crate) fn checked_texture_slot(slot: isize) -> PyResult<usize> {
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

pub(crate) fn validate_shader_defs(shader_defs: u32, shader_def_names: &[String]) -> PyResult<()> {
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

pub(crate) fn uniform_field<M: ShaderMaterialData, F, W>(
    storage: &AssetStorage<M>,
    float_index: usize,
) -> PyResult<W>
where
    F: PackedUniform + 'static,
    W: FromBorrowedStorage<ValueStorage<F>>,
{
    check_packed_uniform::<F>(float_index)?;
    let data: FieldStorage<UniformData> = storage.borrow_field(
        |material| &material.shader_extension().params.data,
        |material| &mut material.shader_extension_mut().params.data,
    )?;
    Ok(data.borrow_resolved_index_as::<F, ValueStorage<F>, W>(
        float_index,
        uniform_ref::<F>,
        uniform_mut::<F>,
    )?)
}
pub(crate) fn replace_texture<M: ShaderMaterialData>(
    storage: &mut AssetStorage<M>,
    slot: usize,
    handle: Option<Handle<Image>>,
) -> PyResult<()> {
    let mut material = storage.as_mut()?;
    if let Some(texture) = material.shader_extension_mut().texture_slot_mut(slot) {
        *texture = handle;
    }
    Ok(())
}
pub(crate) fn _logical_type_id<M: ShaderMaterialData>(
    storage: &AssetStorage<M>,
) -> PyResult<Option<u64>> {
    Ok(storage.as_ref()?.shader_extension().logical_type_id)
}
pub(crate) fn _set_logical_type_id<M: ShaderMaterialData>(
    storage: &mut AssetStorage<M>,
    logical_type_id: u64,
) -> PyResult<()> {
    storage.as_mut()?.shader_extension_mut().logical_type_id = Some(logical_type_id);
    Ok(())
}
pub(crate) fn get_shader_defs<M: ShaderMaterialData>(storage: &AssetStorage<M>) -> PyResult<u32> {
    Ok(storage.as_ref()?.shader_extension().shader_defs)
}
pub(crate) fn set_shader_defs<M: ShaderMaterialData>(
    storage: &mut AssetStorage<M>,
    defs: u32,
) -> PyResult<()> {
    {
        let material = storage.as_ref()?;
        validate_shader_defs(defs, &material.shader_extension().shader_def_names)?;
    }
    storage.as_mut()?.shader_extension_mut().shader_defs = defs;
    Ok(())
}
pub(crate) fn get_texture<M: ShaderMaterialData>(
    storage: &AssetStorage<M>,
    slot: isize,
) -> PyResult<Option<PyHandle>> {
    let slot = checked_texture_slot(slot)?;
    let material = storage.as_ref()?;
    Ok(material
        .shader_extension()
        .texture_slot(slot)
        .and_then(Option::as_ref)
        .map(PyHandle::from))
}
pub(crate) fn set_texture<M: ShaderMaterialData>(
    storage: &mut AssetStorage<M>,
    slot: isize,
    handle: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let slot = checked_texture_slot(slot)?;
    let py_handle = extract_handle_from_any(handle)?;
    let typed: Handle<Image> = (&py_handle).try_into()?;
    replace_texture(storage, slot, Some(typed))
}
pub(crate) fn clear_texture<M: ShaderMaterialData>(
    storage: &mut AssetStorage<M>,
    slot: isize,
) -> PyResult<()> {
    let slot = checked_texture_slot(slot)?;
    replace_texture(storage, slot, None)
}
pub(crate) fn set_data_float<M: ShaderMaterialData>(
    storage: &mut AssetStorage<M>,
    index: isize,
    value: f32,
) -> PyResult<()> {
    let index = checked_index(index, "data index")?;
    if index >= MAX_DATA_FLOATS {
        return Err(PyIndexError::new_err(shader_float_index_out_of_range(
            index,
            MAX_DATA_FLOATS - 1,
        )));
    }
    let mut mat = storage.as_mut()?;
    mat.shader_extension_mut().params.data[index / 4][index % 4] = value;
    Ok(())
}
pub(crate) fn get_data_float<M: ShaderMaterialData>(
    storage: &AssetStorage<M>,
    index: isize,
) -> PyResult<f32> {
    let index = checked_index(index, "data index")?;
    if index >= MAX_DATA_FLOATS {
        return Err(PyIndexError::new_err(shader_float_index_out_of_range(
            index,
            MAX_DATA_FLOATS - 1,
        )));
    }
    let mat = storage.as_ref()?;
    Ok(mat.shader_extension().params.data[index / 4][index % 4])
}
pub(crate) fn set_data_floats<M: ShaderMaterialData>(
    storage: &mut AssetStorage<M>,
    start: isize,
    values: &Bound<'_, PyAny>,
) -> PyResult<()> {
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
    let mut mat = storage.as_mut()?;
    for (i, &val) in values.iter().enumerate() {
        let idx = start + i;
        mat.shader_extension_mut().params.data[idx / 4][idx % 4] = val;
    }
    Ok(())
}
pub(crate) fn borrow_field<M: ShaderMaterialData>(
    storage: &AssetStorage<M>,
    py: Python<'_>,
    float_index: isize,
    value_type: &Bound<'_, PyType>,
) -> PyResult<Py<PyAny>> {
    let index = checked_index(float_index, "float index")?;
    if value_type.is(py.get_type::<PyVec2>()) {
        let value: PyVec2 = uniform_field::<_, Vec2, _>(storage, index)?;
        return Ok(Py::new(py, value)?.into_any());
    }
    if value_type.is(py.get_type::<PyVec3>()) {
        let value: PyVec3 = uniform_field::<_, Vec3, _>(storage, index)?;
        return Ok(Py::new(py, value)?.into_any());
    }
    if value_type.is(py.get_type::<PyVec4>()) {
        let value: PyVec4 = uniform_field::<_, Vec4, _>(storage, index)?;
        return Ok(Py::new(py, value)?.into_any());
    }
    if value_type.is(py.get_type::<PyLinearRgba>()) {
        let value: PyLinearRgba = uniform_field::<_, LinearRgba, _>(storage, index)?;
        return Ok(Py::new(py, value)?.into_any());
    }
    Err(PyTypeError::new_err(unsupported_packed_uniform_type(
        value_type.name()?,
    )))
}
pub(crate) fn get_data_floats<M: ShaderMaterialData>(
    storage: &AssetStorage<M>,
    start: isize,
    count: isize,
) -> PyResult<Vec<f32>> {
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
    let mat = storage.as_ref()?;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let idx = start + i;
        result.push(mat.shader_extension().params.data[idx / 4][idx % 4]);
    }
    Ok(result)
}
