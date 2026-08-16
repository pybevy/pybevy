use std::sync::Arc;

use bevy::ecs::{component::ComponentId, entity::Entity, ptr::OwningPtr, world::World};
use numpy::{PyReadonlyArray1, PyReadonlyArray2};
use pybevy_core::{
    BatchComponent, PreparedBatchComponent,
    batch_columns::{ColumnShape, CustomColumnError, CustomCountAgreement},
    component_batch::{FieldColumn, build_wrapper_rows, column_dtype_for},
    registry::global_registry,
};
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyDict, PyType},
};

use super::{
    component_layout::{ComponentLayout, ComponentLayoutExt, PrimitiveType, PrimitiveTypeExt},
    component_type::register_custom_component,
    component_wrapper::*,
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
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.component_cls)?;
        for (_, array) in &self.field_arrays {
            visit.call(array)?;
        }
        Ok(())
    }

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
            return Err(custom_batch_err(CustomColumnError::NotDecorated {
                class_name: cls.name()?.to_string(),
            }));
        }

        // Validate: must not be PyObject storage
        let has_pyobject_storage = cls
            .getattr("__pybevy_storage__")
            .ok()
            .and_then(|attr| attr.extract::<String>().ok())
            .map(|s| s == "pyobject")
            .unwrap_or(false);
        if has_pyobject_storage {
            return Err(custom_batch_err(CustomColumnError::PyObjectStorage {
                class_name: cls.name()?.to_string(),
            }));
        }

        // Compute layout
        let layout = ComponentLayout::from_annotations(cls)?;

        let kwargs = match kwargs {
            Some(kwargs) if !kwargs.is_empty() => kwargs,
            _ => return Err(custom_batch_err(CustomColumnError::NoKwargs)),
        };

        // Validate and normalize each kwarg through the shared shape rules.
        let np = py.import("numpy")?;
        let mut field_arrays = Vec::new();
        let mut agree = CustomCountAgreement::default();

        for (key, value) in kwargs.iter() {
            let field_name: String = key.extract()?;

            // Find field in layout
            let (field_idx, field_info) = layout
                .fields
                .iter()
                .enumerate()
                .find(|(_, f)| f.name == field_name)
                .ok_or_else(|| {
                    custom_batch_err(CustomColumnError::UnknownField {
                        field: field_name.clone(),
                        component: layout.name.clone(),
                        valid: format!("{:?}", layout.field_names()),
                    })
                })?;

            // Accept real NumPy, the bounded `pybevy.array` array (via its
            // `__array__`), and (nested) lists/tuples of numbers.
            let value = np.call_method1("asarray", (value,))?;

            // Validate array shape against the shared custom-batch rules.
            let ndim: usize = value.getattr("ndim")?.extract()?;
            let shape: Vec<usize> = value.getattr("shape")?.extract()?;
            let (_, cols) = column_dtype_for(field_info.field_type);
            let type_name = format!("{:?}", field_info.field_type);
            let shape_rule = if field_info.field_type.is_composite() {
                ColumnShape::CustomComposite {
                    field: &field_name,
                    type_name: &type_name,
                    cols,
                }
            } else {
                ColumnShape::CustomScalar { field: &field_name }
            };
            let length = shape_rule
                .plan(ndim, &shape)
                .map_err(|error| PyValueError::new_err(error.to_string()))?;
            agree
                .observe(&field_name, length)
                .map_err(|error| PyValueError::new_err(error.to_string()))?;

            // Cast to correct numpy dtype
            let target_dtype = field_info.field_type.to_numpy_dtype();
            let dtype_obj = np.call_method1("dtype", (target_dtype,))?;
            let arr = np.call_method1("ascontiguousarray", (&value,))?;
            let arr = arr.call_method1("astype", (&dtype_obj,))?;

            field_arrays.push((field_idx, arr.unbind()));
        }

        // kwargs is non-empty, so at least one column was observed.
        let count = agree.count().unwrap_or(0);

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

/// Map a neutral custom-batch validation error to the exception type pyo3
/// raises: `TypeError` for the not-decorated and PyObject-storage variants,
/// `ValueError` for the rest.
fn custom_batch_err(error: CustomColumnError) -> PyErr {
    if error.is_type_error() {
        PyTypeError::new_err(error.to_string())
    } else {
        PyValueError::new_err(error.to_string())
    }
}

struct PreparedCustomBatch {
    wrapper_size: WrapperSize,
    data_size: usize,
    rows: Vec<Vec<u8>>,
}

impl PreparedBatchComponent for PreparedCustomBatch {
    fn count(&self) -> usize {
        self.rows.len()
    }

    fn insert(&mut self, component_id: ComponentId, entities: &[Entity], world: &mut World) {
        assert_eq!(
            self.rows.len(),
            entities.len(),
            "validated custom batch count changed before commit"
        );

        for (entity_id, buffer) in entities.iter().copied().zip(self.rows.drain(..)) {
            macro_rules! insert_wrapper {
                ($size:expr, $wrapper_type:ty) => {
                    if self.wrapper_size == $size {
                        let mut wrapper = <$wrapper_type>::default();
                        let copy_len = self.data_size.min(wrapper.data.len());
                        wrapper.data[..copy_len].copy_from_slice(&buffer[..copy_len]);

                        OwningPtr::make(wrapper, |ptr| {
                            // SAFETY: preparation validated the layout and selected the wrapper
                            // registered for this custom component's exact ComponentId.
                            unsafe {
                                world.entity_mut(entity_id).insert_by_id(component_id, ptr);
                            }
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
    }
}

fn prepare_custom_batch(
    py: Python,
    batch: &PyCustomComponentBatch,
) -> PyResult<PreparedCustomBatch> {
    let layout = &batch.layout;
    let mut holders = Vec::with_capacity(batch.field_arrays.len());
    for (field_idx, array) in &batch.field_arrays {
        let array = array.bind(py);
        let field_info = &layout.fields[*field_idx];
        let holder = match field_info.field_type {
            PrimitiveType::F32 => {
                ReadonlyArrayHolder::F32(array.extract::<PyReadonlyArray1<f32>>()?)
            }
            PrimitiveType::F64 => {
                ReadonlyArrayHolder::F64(array.extract::<PyReadonlyArray1<f64>>()?)
            }
            PrimitiveType::I32 => {
                ReadonlyArrayHolder::I32(array.extract::<PyReadonlyArray1<i32>>()?)
            }
            PrimitiveType::I64 => {
                ReadonlyArrayHolder::I64(array.extract::<PyReadonlyArray1<i64>>()?)
            }
            PrimitiveType::U32 => {
                ReadonlyArrayHolder::U32(array.extract::<PyReadonlyArray1<u32>>()?)
            }
            PrimitiveType::U64 => {
                ReadonlyArrayHolder::U64(array.extract::<PyReadonlyArray1<u64>>()?)
            }
            PrimitiveType::Bool => {
                ReadonlyArrayHolder::Bool(array.extract::<PyReadonlyArray1<u8>>()?)
            }
            PrimitiveType::Vec3 => {
                ReadonlyArrayHolder::Vec3(array.extract::<PyReadonlyArray2<f32>>()?)
            }
            PrimitiveType::Vec2 => {
                ReadonlyArrayHolder::Vec2(array.extract::<PyReadonlyArray2<f32>>()?)
            }
        };
        holders.push(holder);
    }

    let mut columns: Vec<(usize, FieldColumn<'_>)> = Vec::with_capacity(holders.len());
    for ((field_idx, _), holder) in batch.field_arrays.iter().zip(&holders) {
        let column = match holder {
            ReadonlyArrayHolder::F32(array) => {
                FieldColumn::F32(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
            ReadonlyArrayHolder::F64(array) => {
                FieldColumn::F64(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
            ReadonlyArrayHolder::I32(array) => {
                FieldColumn::I32(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
            ReadonlyArrayHolder::I64(array) => {
                FieldColumn::I64(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
            ReadonlyArrayHolder::U32(array) => {
                FieldColumn::U32(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
            ReadonlyArrayHolder::U64(array) => {
                FieldColumn::U64(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
            ReadonlyArrayHolder::Bool(array) => {
                FieldColumn::Bool(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
            ReadonlyArrayHolder::Vec3(array) => {
                FieldColumn::Vec3(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
            ReadonlyArrayHolder::Vec2(array) => {
                FieldColumn::Vec2(array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?)
            }
        };
        columns.push((*field_idx, column));
    }

    let rows = build_wrapper_rows(layout, &columns, batch.count);

    Ok(PreparedCustomBatch {
        wrapper_size: layout.wrapper_size,
        data_size: layout.data_size,
        rows,
    })
}

/// Bridge for CustomComponentBatch that implements BatchComponent.
pub struct CustomComponentBatchBridge;

impl BatchComponent for CustomComponentBatchBridge {
    fn name(&self) -> &'static str {
        "CustomComponentBatch"
    }

    fn component_type_ptr(&self, py: Python, batch: &Bound<PyAny>) -> PyResult<usize> {
        let batch = batch.extract::<PyRef<PyCustomComponentBatch>>()?;
        Ok(batch.component_cls.bind(py).as_type_ptr() as usize)
    }

    fn count(&self, _py: Python, batch: &Bound<PyAny>) -> PyResult<usize> {
        let batch = batch.extract::<PyRef<PyCustomComponentBatch>>()?;
        Ok(batch.count)
    }

    fn prepare(
        &self,
        py: Python,
        batch: &Bound<PyAny>,
    ) -> PyResult<Box<dyn PreparedBatchComponent>> {
        let batch = batch.extract::<PyRef<PyCustomComponentBatch>>()?;
        Ok(Box::new(prepare_custom_batch(py, &batch)?))
    }

    fn insert_bulk(
        &self,
        py: Python,
        batch: &Bound<PyAny>,
        entities: &[Entity],
        world: &mut World,
    ) -> PyResult<()> {
        let batch = batch.extract::<PyRef<PyCustomComponentBatch>>()?;
        let type_ptr = batch.component_cls.bind(py).as_type_ptr();
        let component_id = register_custom_component(world, type_ptr, py);
        let mut prepared = prepare_custom_batch(py, &batch)?;
        prepared.insert(component_id, entities, world);

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
