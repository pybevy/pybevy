use std::sync::Arc;

use bevy::ecs::{entity::Entity, ptr::OwningPtr, world::World};
use numpy::{PyReadonlyArray1, PyReadonlyArray2};
use pybevy_core::{BatchComponent, registry::global_registry};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyDict, PyType},
};

use super::{
    component_layout::{ComponentLayout, FieldInfo, PrimitiveType},
    component_type::register_custom_component,
    component_wrapper::*,
    helpers::type_utils::get_python_type_name,
};

/// Batch component for custom Python @component classes.
///
/// Created via `MyComponent.from_numpy(x=xs, y=ys)` where xs, ys are numpy arrays.
/// Stores the component layout and per-field numpy arrays, then bulk-inserts into
/// wrapper storage during spawn_batch.
#[pyclass(name = "CustomComponentBatch")]
pub struct PyCustomComponentBatch {
    /// Python class for type identity and registration
    component_cls: Py<PyType>,
    /// Layout describing field offsets, types, and wrapper size
    layout: ComponentLayout,
    /// (field_index, numpy array) pairs for each specified field
    field_arrays: Vec<(usize, Py<PyAny>)>,
    /// Number of entities to spawn
    count: usize,
    /// Qualified name for the component (retained for debugging)
    #[allow(dead_code)]
    qualified_name: String,
}

#[pymethods]
impl PyCustomComponentBatch {
    #[new]
    #[pyo3(signature = (cls, **kwargs))]
    fn new(
        py: Python,
        cls: &Bound<'_, PyType>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        // Validate: must be @component decorated
        let has_decorator = cls
            .getattr("__pybevy_component_decorated__")
            .ok()
            .and_then(|marker| marker.is_truthy().ok())
            .unwrap_or(false);
        if !has_decorator {
            return Err(PyTypeError::new_err(format!(
                "Class '{}' must be decorated with @component",
                cls.name()?
            )));
        }

        // Validate: must not be PyObject storage
        let has_pyobject_storage = cls
            .getattr("__pybevy_storage__")
            .ok()
            .and_then(|attr| attr.extract::<String>().ok())
            .map(|s| s == "pyobject")
            .unwrap_or(false);
        if has_pyobject_storage {
            return Err(PyTypeError::new_err(format!(
                "from_numpy() is not supported for components with storage=\"python\". \
                 '{}' uses PyObject storage which cannot be batch-spawned from numpy arrays.",
                cls.name()?
            )));
        }

        // Compute layout
        let layout = ComponentLayout::from_annotations(cls)?;

        let kwargs = kwargs.ok_or_else(|| {
            PyValueError::new_err("from_numpy() requires at least one keyword argument")
        })?;

        if kwargs.is_empty() {
            return Err(PyValueError::new_err(
                "from_numpy() requires at least one keyword argument",
            ));
        }

        // Validate and normalize each kwarg
        let np = py.import("numpy")?;
        let mut field_arrays = Vec::new();
        let mut expected_count: Option<(usize, String)> = None;

        for (key, value) in kwargs.iter() {
            let field_name: String = key.extract()?;

            // Find field in layout
            let (field_idx, field_info) = layout
                .fields
                .iter()
                .enumerate()
                .find(|(_, f)| f.name == field_name)
                .ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "Unknown field '{}' for component '{}'. Valid fields: {:?}",
                        field_name,
                        layout.name,
                        layout.field_names()
                    ))
                })?;

            // Validate array shape based on field type
            let ndim: usize = value.getattr("ndim")?.extract()?;
            let length: usize = if field_info.field_type.is_composite() {
                // Vec3/Vec2: require 2D array with shape (N, 3) or (N, 2)
                let expected_cols = field_info.field_type.element_count();
                if ndim != 2 {
                    return Err(PyValueError::new_err(format!(
                        "Field '{}' ({:?}): expected 2D array with shape (N, {}), got {}D array",
                        field_name,
                        field_info.field_type,
                        expected_cols,
                        ndim
                    )));
                }
                let shape: Vec<usize> = value.getattr("shape")?.extract()?;
                if shape[1] != expected_cols {
                    return Err(PyValueError::new_err(format!(
                        "Field '{}' ({:?}): expected shape (N, {}), got (N, {})",
                        field_name,
                        field_info.field_type,
                        expected_cols,
                        shape[1]
                    )));
                }
                shape[0]
            } else {
                // Scalar: require 1D array
                if ndim != 1 {
                    return Err(PyValueError::new_err(format!(
                        "Field '{}' must be a 1D array, got {}D",
                        field_name, ndim
                    )));
                }
                value.len()?
            };
            if let Some((prev_count, ref prev_name)) = expected_count {
                if length != prev_count {
                    return Err(PyValueError::new_err(format!(
                        "Array length mismatch: '{}' has {} elements but '{}' has {}",
                        prev_name, prev_count, field_name, length
                    )));
                }
            } else {
                expected_count = Some((length, field_name.clone()));
            }

            // Cast to correct numpy dtype
            let target_dtype = field_info.field_type.to_numpy_dtype();
            let dtype_obj = np.call_method1("dtype", (target_dtype,))?;
            let arr = np.call_method1("ascontiguousarray", (&value,))?;
            let arr = arr.call_method1("astype", (&dtype_obj,))?;

            field_arrays.push((field_idx, arr.unbind()));
        }

        let count = expected_count
            .map(|(c, _)| c)
            .ok_or_else(|| PyValueError::new_err("No arrays provided"))?;

        // Qualified name for registration
        let module = cls
            .getattr("__module__")
            .ok()
            .and_then(|m| m.extract::<String>().ok())
            .unwrap_or_default();
        let qualname = cls
            .getattr("__qualname__")
            .ok()
            .and_then(|q| q.extract::<String>().ok())
            .unwrap_or_default();
        let qualified_name = format!("{}.{}", module, qualname);

        Ok(PyCustomComponentBatch {
            component_cls: cls.clone().unbind(),
            layout,
            field_arrays,
            count,
            qualified_name,
        })
    }
}

/// Enum to hold borrowed numpy arrays of different dtypes,
/// keeping the PyReadonlyArray1 alive during the entity insertion loop.
enum ReadonlyArrayHolder<'py> {
    F32(PyReadonlyArray1<'py, f32>),
    F64(PyReadonlyArray1<'py, f64>),
    I32(PyReadonlyArray1<'py, i32>),
    I64(PyReadonlyArray1<'py, i64>),
    U32(PyReadonlyArray1<'py, u32>),
    U64(PyReadonlyArray1<'py, u64>),
    Bool(PyReadonlyArray1<'py, u8>),
    /// 2D array (N, 3) for Vec3 fields
    Vec3(PyReadonlyArray2<'py, f32>),
    /// 2D array (N, 2) for Vec2 fields
    Vec2(PyReadonlyArray2<'py, f32>),
}

/// A borrowed slice from a ReadonlyArrayHolder, typed by primitive type.
enum FieldSlice<'a> {
    F32(&'a [f32]),
    F64(&'a [f64]),
    I32(&'a [i32]),
    I64(&'a [i64]),
    U32(&'a [u32]),
    U64(&'a [u64]),
    Bool(&'a [u8]),
    /// Flat slice of f32 from (N, 3) array — stride 3 per entity
    Vec3(&'a [f32]),
    /// Flat slice of f32 from (N, 2) array — stride 2 per entity
    Vec2(&'a [f32]),
}

impl<'a> FieldSlice<'a> {
    /// Write the value at `index` into `buffer` at the given byte offset.
    #[inline(always)]
    fn write_to_buffer(&self, index: usize, buffer: &mut [u8], offset: usize) {
        match self {
            FieldSlice::F32(s) => {
                buffer[offset..offset + 4].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldSlice::F64(s) => {
                buffer[offset..offset + 8].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldSlice::I32(s) => {
                buffer[offset..offset + 4].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldSlice::I64(s) => {
                buffer[offset..offset + 8].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldSlice::U32(s) => {
                buffer[offset..offset + 4].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldSlice::U64(s) => {
                buffer[offset..offset + 8].copy_from_slice(&s[index].to_le_bytes());
            }
            FieldSlice::Bool(s) => {
                buffer[offset] = s[index];
            }
            FieldSlice::Vec3(s) => {
                // 3 contiguous f32 per entity
                let base = index * 3;
                buffer[offset..offset + 4].copy_from_slice(&s[base].to_le_bytes());
                buffer[offset + 4..offset + 8].copy_from_slice(&s[base + 1].to_le_bytes());
                buffer[offset + 8..offset + 12].copy_from_slice(&s[base + 2].to_le_bytes());
            }
            FieldSlice::Vec2(s) => {
                // 2 contiguous f32 per entity
                let base = index * 2;
                buffer[offset..offset + 4].copy_from_slice(&s[base].to_le_bytes());
                buffer[offset + 4..offset + 8].copy_from_slice(&s[base + 1].to_le_bytes());
            }
        }
    }
}

/// Bridge for CustomComponentBatch that implements BatchComponent.
pub struct CustomComponentBatchBridge;

impl BatchComponent for CustomComponentBatchBridge {
    fn name(&self) -> &'static str {
        "CustomComponentBatch"
    }

    fn count(&self, _py: Python, batch: &Bound<PyAny>) -> PyResult<usize> {
        let batch = batch.extract::<PyRef<PyCustomComponentBatch>>()?;
        Ok(batch.count)
    }

    fn insert_bulk(
        &self,
        py: Python,
        batch: &Bound<PyAny>,
        entities: &[Entity],
        world: &mut World,
    ) -> PyResult<()> {
        let batch = batch.extract::<PyRef<PyCustomComponentBatch>>()?;
        let layout = &batch.layout;

        // Register the component and get its ComponentId
        let type_ptr = batch.component_cls.bind(py).as_type_ptr();
        let name = get_python_type_name(py, type_ptr);
        let component_id = register_custom_component(world, type_ptr, name);

        // Borrow all field arrays as typed slices — zero-copy, held alive for loop
        let mut holders: Vec<ReadonlyArrayHolder<'_>> =
            Vec::with_capacity(batch.field_arrays.len());
        for (_, arr) in &batch.field_arrays {
            let arr_bound = arr.bind(py);
            let field_idx = batch
                .field_arrays
                .iter()
                .position(|(_, a)| std::ptr::eq(a, arr))
                .unwrap();
            let (actual_field_idx, _) = batch.field_arrays[field_idx];
            let field_info = &layout.fields[actual_field_idx];

            let holder = match field_info.field_type {
                PrimitiveType::F32 => {
                    ReadonlyArrayHolder::F32(arr_bound.extract::<PyReadonlyArray1<f32>>()?)
                }
                PrimitiveType::F64 => {
                    ReadonlyArrayHolder::F64(arr_bound.extract::<PyReadonlyArray1<f64>>()?)
                }
                PrimitiveType::I32 => {
                    ReadonlyArrayHolder::I32(arr_bound.extract::<PyReadonlyArray1<i32>>()?)
                }
                PrimitiveType::I64 => {
                    ReadonlyArrayHolder::I64(arr_bound.extract::<PyReadonlyArray1<i64>>()?)
                }
                PrimitiveType::U32 => {
                    ReadonlyArrayHolder::U32(arr_bound.extract::<PyReadonlyArray1<u32>>()?)
                }
                PrimitiveType::U64 => {
                    ReadonlyArrayHolder::U64(arr_bound.extract::<PyReadonlyArray1<u64>>()?)
                }
                PrimitiveType::Bool => {
                    ReadonlyArrayHolder::Bool(arr_bound.extract::<PyReadonlyArray1<u8>>()?)
                }
                PrimitiveType::Vec3 => {
                    ReadonlyArrayHolder::Vec3(arr_bound.extract::<PyReadonlyArray2<f32>>()?)
                }
                PrimitiveType::Vec2 => {
                    ReadonlyArrayHolder::Vec2(arr_bound.extract::<PyReadonlyArray2<f32>>()?)
                }
            };
            holders.push(holder);
        }

        // Extract slices from holders
        let mut field_slices: Vec<(&FieldInfo, FieldSlice<'_>)> = Vec::with_capacity(holders.len());
        for (idx, holder) in holders.iter().enumerate() {
            let (field_idx, _) = batch.field_arrays[idx];
            let field_info = &layout.fields[field_idx];
            let slice =
                match holder {
                    ReadonlyArrayHolder::F32(a) => FieldSlice::F32(a.as_slice().map_err(|e| {
                        PyValueError::new_err(format!("Array not contiguous: {}", e))
                    })?),
                    ReadonlyArrayHolder::F64(a) => FieldSlice::F64(a.as_slice().map_err(|e| {
                        PyValueError::new_err(format!("Array not contiguous: {}", e))
                    })?),
                    ReadonlyArrayHolder::I32(a) => FieldSlice::I32(a.as_slice().map_err(|e| {
                        PyValueError::new_err(format!("Array not contiguous: {}", e))
                    })?),
                    ReadonlyArrayHolder::I64(a) => FieldSlice::I64(a.as_slice().map_err(|e| {
                        PyValueError::new_err(format!("Array not contiguous: {}", e))
                    })?),
                    ReadonlyArrayHolder::U32(a) => FieldSlice::U32(a.as_slice().map_err(|e| {
                        PyValueError::new_err(format!("Array not contiguous: {}", e))
                    })?),
                    ReadonlyArrayHolder::U64(a) => FieldSlice::U64(a.as_slice().map_err(|e| {
                        PyValueError::new_err(format!("Array not contiguous: {}", e))
                    })?),
                    ReadonlyArrayHolder::Bool(a) => {
                        FieldSlice::Bool(a.as_slice().map_err(|e| {
                            PyValueError::new_err(format!("Array not contiguous: {}", e))
                        })?)
                    }
                    ReadonlyArrayHolder::Vec3(a) => {
                        FieldSlice::Vec3(a.as_slice().map_err(|e| {
                            PyValueError::new_err(format!("Array not contiguous: {}", e))
                        })?)
                    }
                    ReadonlyArrayHolder::Vec2(a) => {
                        FieldSlice::Vec2(a.as_slice().map_err(|e| {
                            PyValueError::new_err(format!("Array not contiguous: {}", e))
                        })?)
                    }
                };
            field_slices.push((field_info, slice));
        }

        let wrapper_size = layout.wrapper_size;
        let data_size = layout.data_size;

        // Tight loop: write field bytes into wrapper buffer and insert via insert_by_id
        for (i, &entity_id) in entities.iter().enumerate() {
            // Create zero-initialized buffer
            let mut buffer = vec![0u8; wrapper_size.size_bytes()];

            // Write each field's value
            for (field_info, slice) in &field_slices {
                slice.write_to_buffer(i, &mut buffer, field_info.offset);
            }

            // Insert using the appropriate wrapper type
            macro_rules! insert_wrapper {
                ($size:expr, $wrapper_type:ty) => {
                    if wrapper_size == $size {
                        let mut wrapper = <$wrapper_type>::default();
                        let copy_len = data_size.min(wrapper.data.len());
                        wrapper.data[..copy_len].copy_from_slice(&buffer[..copy_len]);

                        OwningPtr::make(wrapper, |ptr| unsafe {
                            world.entity_mut(entity_id).insert_by_id(component_id, ptr);
                        });
                    }
                };
            }

            insert_wrapper!(WrapperSize::W8, ComponentWrapper8);
            insert_wrapper!(WrapperSize::W16, ComponentWrapper16);
            insert_wrapper!(WrapperSize::W32, ComponentWrapper32);
            insert_wrapper!(WrapperSize::W64, ComponentWrapper64);
            insert_wrapper!(WrapperSize::W128, ComponentWrapper128);
            insert_wrapper!(WrapperSize::W256, ComponentWrapper256);
            insert_wrapper!(WrapperSize::W512, ComponentWrapper512);
            insert_wrapper!(WrapperSize::W1024, ComponentWrapper1024);
        }

        Ok(())
    }
}

/// Register the CustomComponentBatch bridge so spawn_batch can detect it.
pub fn register_custom_batch_bridge() {
    Python::attach(|py| {
        let ptr = <PyCustomComponentBatch as pyo3::PyTypeInfo>::type_object(py).as_type_ptr();
        global_registry::register_batch_bridge(ptr, Arc::new(CustomComponentBatchBridge));
    });
}
