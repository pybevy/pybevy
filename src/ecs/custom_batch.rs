use std::sync::Arc;

use bevy::ecs::{component::ComponentId, entity::Entity, ptr::OwningPtr, world::World};
use numpy::{PyReadonlyArray1, PyReadonlyArray2};
use pybevy_array::{ArrayDType, owned_contiguous_le_bytes_from_any};
use pybevy_core::{
    BatchComponent, PreparedBatchComponent,
    batch_columns::{
        ColumnDType, ColumnData, ColumnShape, CustomColumnError, CustomCountAgreement,
        IntegerColumn,
    },
    check_integer_batch_column,
    component_batch::{FieldColumn, build_wrapper_rows, column_dtype_for, field_column_for},
    registry::global_registry,
};
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyModuleNotFoundError, PyTypeError, PyValueError},
    intern,
    prelude::*,
    types::{PyBool, PyBytes, PyDict, PyList, PyTuple, PyType},
};

use super::{
    component_layout::{
        ComponentLayout, ComponentStorageType, ComponentStorageTypeExt, PrimitiveType,
        PrimitiveTypeExt, PrimitiveValue, extract_primitive_from_py, primitive_to_py,
    },
    component_type::{PreparedCustomComponentRegistration, register_prepared_custom_component},
    component_wrapper::*,
};

/// Batch component for custom Python @component classes.
///
/// Created via `MyComponent.batch(x=xs, y=ys)` where xs and ys are arrays or
/// sequences. Stores owned portable columns or normalized NumPy arrays, then
/// bulk-inserts into wrapper storage during spawn_batch.
#[pyclass(name = "CustomComponentBatch")]
pub struct PyCustomComponentBatch {
    /// Python class for type identity and registration
    component_cls: Py<PyType>,
    /// Layout describing field offsets, types, and wrapper size
    layout: ComponentLayout,
    /// (field_index, numpy array) pairs for each specified field
    field_arrays: Vec<(usize, Py<PyAny>)>,
    /// Portable columns copied into owned storage before enqueue.
    field_values: Vec<(usize, ColumnData)>,
    /// Number of entities to spawn
    count: usize,
    /// Qualified name for the component (retained for debugging)
    #[allow(dead_code)]
    qualified_name: String,
}

enum FieldDefault<'py> {
    Value(Bound<'py, PyAny>),
    Factory(Bound<'py, PyAny>),
}

impl<'py> FieldDefault<'py> {
    fn next_value(&self) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Value(value) => Ok(value.clone()),
            Self::Factory(factory) => factory.call0(),
        }
    }
}

fn field_default<'py>(
    cls: &Bound<'py, PyType>,
    dataclass_fields: Option<&Bound<'py, PyDict>>,
    missing: &Bound<'py, PyAny>,
    field_name: &str,
) -> PyResult<FieldDefault<'py>> {
    if let Some(fields) = dataclass_fields
        && let Some(field) = fields.get_item(field_name)?
    {
        let default = field.getattr("default")?;
        if !default.is(missing) {
            return Ok(FieldDefault::Value(default));
        }
        let factory = field.getattr("default_factory")?;
        if !factory.is(missing) {
            return Ok(FieldDefault::Factory(factory));
        }
    } else if let Some(default) = cls.getattr_opt(field_name)? {
        return Ok(FieldDefault::Value(default));
    }

    Err(PyTypeError::new_err(format!(
        "{}.batch() omits required field '{field_name}'",
        cls.name()?
    )))
}

fn empty_column(field_type: PrimitiveType) -> ColumnData {
    match field_type {
        PrimitiveType::F32
        | PrimitiveType::Vec2
        | PrimitiveType::Vec3
        | PrimitiveType::Vec4
        | PrimitiveType::Quat
        | PrimitiveType::Mat2
        | PrimitiveType::Mat3
        | PrimitiveType::Mat4 => ColumnData::F32(Vec::new()),
        PrimitiveType::F64 => ColumnData::F64(Vec::new()),
        PrimitiveType::I32 => ColumnData::I32(Vec::new()),
        PrimitiveType::I64 => ColumnData::I64(Vec::new()),
        PrimitiveType::U32 => ColumnData::U32(Vec::new()),
        PrimitiveType::U64 => ColumnData::U64(Vec::new()),
        PrimitiveType::Bool => ColumnData::Bool(Vec::new()),
    }
}

fn push_value(column: &mut ColumnData, value: PrimitiveValue) {
    match (column, value) {
        (ColumnData::F32(values), PrimitiveValue::F32(value)) => values.push(value),
        (ColumnData::F64(values), PrimitiveValue::F64(value)) => values.push(value),
        (ColumnData::I32(values), PrimitiveValue::I32(value)) => values.push(value),
        (ColumnData::I64(values), PrimitiveValue::I64(value)) => values.push(value),
        (ColumnData::U32(values), PrimitiveValue::U32(value)) => values.push(value),
        (ColumnData::U64(values), PrimitiveValue::U64(value)) => values.push(value),
        (ColumnData::Bool(values), PrimitiveValue::Bool(value)) => values.push(u8::from(value)),
        (ColumnData::F32(values), composite) if composite.f32_lanes().is_some() => {
            let (lanes, count) = composite.f32_lanes().expect("checked composite value");
            values.extend_from_slice(&lanes[..count]);
        }
        _ => unreachable!("primitive extraction returned a different field type"),
    }
}

fn default_column(
    field_type: PrimitiveType,
    source: &FieldDefault<'_>,
    count: usize,
) -> PyResult<ColumnData> {
    let mut column = empty_column(field_type);
    for _ in 0..count {
        let value = source.next_value()?;
        push_value(&mut column, extract_primitive_from_py(field_type, &value)?);
    }
    Ok(column)
}

/// Whether batch rows must run the class's own initializer: anything other
/// than a dataclass-generated `__init__` without `__post_init__`.
fn constructs_rows(cls: &Bound<'_, PyType>) -> PyResult<bool> {
    if cls.hasattr(intern!(cls.py(), "__post_init__"))? {
        return Ok(true);
    }
    let Some(code) = cls
        .getattr(intern!(cls.py(), "__init__"))?
        .getattr_opt(intern!(cls.py(), "__code__"))?
    else {
        return Ok(false);
    };
    let qualname: String = code.getattr(intern!(cls.py(), "co_qualname"))?.extract()?;
    let generated = qualname == DATACLASS_INIT_QUALNAME
        && cls.hasattr(intern!(cls.py(), "__dataclass_fields__"))?;
    Ok(!generated)
}

const DATACLASS_INIT_QUALNAME: &str = "__create_fn__.<locals>.__init__";

fn numpy_column(field_type: PrimitiveType, array: &Bound<'_, PyAny>) -> PyResult<ColumnData> {
    let bytes = array.call_method0(intern!(array.py(), "tobytes"))?;
    ColumnData::from_le_bytes(
        column_dtype_for(field_type).0,
        bytes.cast::<PyBytes>()?.as_bytes(),
    )
    .ok_or_else(|| PyValueError::new_err("custom batch column has incomplete elements"))
}

fn column_value<'py>(
    py: Python<'py>,
    field_type: PrimitiveType,
    column: &ColumnData,
    row: usize,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(match (field_type, column) {
        (composite, ColumnData::F32(values)) if composite.is_composite() => {
            let lanes = composite.element_count();
            let value =
                PrimitiveValue::from_f32_lanes(composite, &values[row * lanes..(row + 1) * lanes])
                    .expect("composite lanes match their field type");
            primitive_to_py(py, value)?
        }
        (_, ColumnData::F32(values)) => values[row].into_pyobject(py)?.into_any(),
        (_, ColumnData::F64(values)) => values[row].into_pyobject(py)?.into_any(),
        (_, ColumnData::I32(values)) => values[row].into_pyobject(py)?.into_any(),
        (_, ColumnData::I64(values)) => values[row].into_pyobject(py)?.into_any(),
        (_, ColumnData::U32(values)) => values[row].into_pyobject(py)?.into_any(),
        (_, ColumnData::U64(values)) => values[row].into_pyobject(py)?.into_any(),
        (_, ColumnData::Bool(values)) => PyBool::new(py, values[row] != 0).to_owned().into_any(),
    })
}

/// Build every row through the class constructor, as `cls(**row)` would, and
/// read the stored fields back with the single-component extraction rules.
fn construct_rows(
    cls: &Bound<'_, PyType>,
    layout: &ComponentLayout,
    provided: &[(usize, ColumnData)],
    count: usize,
) -> PyResult<Vec<(usize, ColumnData)>> {
    let py = cls.py();
    let mut columns: Vec<ColumnData> = layout
        .fields
        .iter()
        .map(|field| empty_column(field.field_type))
        .collect();
    for row in 0..count {
        let kwargs = PyDict::new(py);
        for (field_idx, column) in provided {
            let field = &layout.fields[*field_idx];
            kwargs.set_item(
                &field.name,
                column_value(py, field.field_type, column, row)?,
            )?;
        }
        let instance = cls.call((), Some(&kwargs))?;
        for (field, column) in layout.fields.iter().zip(&mut columns) {
            let value = instance.getattr(field.name.as_str())?;
            push_value(column, extract_primitive_from_py(field.field_type, &value)?);
        }
    }
    Ok(columns.into_iter().enumerate().collect())
}

impl PyCustomComponentBatch {
    pub(crate) fn prepared_registration(
        &self,
        py: Python<'_>,
    ) -> PyResult<PreparedCustomComponentRegistration> {
        let registration = PreparedCustomComponentRegistration::from_python_class_with_layout(
            self.component_cls.bind(py),
            ComponentStorageType::Wrapper(self.layout.wrapper_size),
            Some(Arc::new(self.layout.clone())),
        )?;
        registration.validate_current(py)?;
        Ok(registration)
    }
}

#[pymethods]
impl PyCustomComponentBatch {
    /// The Batchable protocol's method: the number of entities in this batch.
    fn count(&self) -> usize {
        self.count
    }

    fn __len__(&self) -> usize {
        self.count
    }

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

        let (storage, layout) = ComponentStorageType::storage_with_layout(cls)?;
        if storage == ComponentStorageType::PyObject {
            return Err(custom_batch_err(CustomColumnError::PyObjectStorage {
                class_name: cls.name()?.to_string(),
            }));
        }
        let layout = layout.expect("wrapper storage carries a component layout");

        let kwargs = match kwargs {
            Some(kwargs) if !kwargs.is_empty() => kwargs,
            _ => return Err(custom_batch_err(CustomColumnError::NoKwargs)),
        };

        // Validate and normalize each kwarg through the shared shape rules.
        let mut field_arrays = Vec::new();
        let mut field_values = Vec::new();
        let mut agree = CustomCountAgreement::default();
        let array_type = py.import("pybevy.array")?.getattr("Array")?;

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

            let (column_dtype, cols) = column_dtype_for(field_info.field_type);
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
            if let Some(integer) = IntegerColumn::from_dtype(column_dtype) {
                let name = format!("{}.{field_name}", layout.name);
                check_integer_batch_column(&value, &name, integer)?;
            }
            let is_sequence = value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>();
            let is_bounded_array = value.is_instance(&array_type)?;
            if (is_sequence || is_bounded_array)
                && let Some(target_dtype) = portable_dtype_for(column_dtype)
            {
                let (dtype, shape, bytes) =
                    owned_contiguous_le_bytes_from_any(&value, Some(target_dtype))?;
                let length = shape_rule
                    .plan(shape.len(), &shape)
                    .map_err(|error| PyValueError::new_err(error.to_string()))?;
                agree
                    .observe(&field_name, length)
                    .map_err(|error| PyValueError::new_err(error.to_string()))?;
                if dtype != target_dtype {
                    return Err(PyTypeError::new_err(
                        "custom batch column dtype changed during conversion",
                    ));
                }
                let column = ColumnData::from_le_bytes(column_dtype, &bytes).ok_or_else(|| {
                    PyValueError::new_err("custom batch column has incomplete elements")
                })?;
                field_values.push((field_idx, column));
            } else {
                if !value.hasattr("__array__")? {
                    return Err(unsupported_column_error(&field_name));
                }
                let np = py.import("numpy").map_err(|error| {
                    if error.is_instance_of::<PyModuleNotFoundError>(py) {
                        unsupported_column_error(&field_name)
                    } else {
                        error
                    }
                })?;
                let value = np.call_method1("asarray", (value,))?;
                let ndim: usize = value.getattr("ndim")?.extract()?;
                let shape: Vec<usize> = value.getattr("shape")?.extract()?;
                let length = shape_rule
                    .plan(ndim, &shape)
                    .map_err(|error| PyValueError::new_err(error.to_string()))?;
                agree
                    .observe(&field_name, length)
                    .map_err(|error| PyValueError::new_err(error.to_string()))?;

                let target_dtype = field_info.field_type.to_numpy_dtype();
                let dtype_obj = np.call_method1("dtype", (target_dtype,))?;
                let arr = np.call_method1("ascontiguousarray", (&value,))?;
                let mut arr = arr.call_method1("astype", (&dtype_obj,))?;
                if field_info.field_type == PrimitiveType::Bool {
                    arr = arr.call_method1("view", ("u1",))?;
                }
                field_arrays.push((field_idx, arr.unbind()));
            }
        }

        // kwargs is non-empty, so at least one column was observed.
        let count = agree.count().unwrap_or(0);

        let (field_arrays, field_values) = if constructs_rows(cls)? {
            let mut provided = field_values;
            for (field_idx, array) in field_arrays {
                let field_type = layout.fields[field_idx].field_type;
                provided.push((field_idx, numpy_column(field_type, array.bind(py))?));
            }
            (Vec::new(), construct_rows(cls, &layout, &provided, count)?)
        } else {
            let mut field_values = field_values;
            let mut specified = vec![false; layout.fields.len()];
            for (field_idx, _) in &field_arrays {
                specified[*field_idx] = true;
            }
            for (field_idx, _) in &field_values {
                specified[*field_idx] = true;
            }
            if specified.iter().any(|present| !present) {
                let dataclass_fields = cls.getattr_opt("__dataclass_fields__")?;
                let dataclass_fields = dataclass_fields
                    .as_ref()
                    .map(|fields| fields.cast::<PyDict>())
                    .transpose()?;
                let missing = py.import("dataclasses")?.getattr("MISSING")?;
                for (field_idx, field) in layout.fields.iter().enumerate() {
                    if specified[field_idx] {
                        continue;
                    }
                    let source = field_default(cls, dataclass_fields, &missing, &field.name)?;
                    let column = default_column(field.field_type, &source, count)?;
                    field_values.push((field_idx, column));
                }
            }
            (field_arrays, field_values)
        };

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
            field_values,
            count,
            qualified_name,
        })
    }
}

fn unsupported_column_error(field_name: &str) -> PyErr {
    PyTypeError::new_err(format!(
        "custom batch field '{field_name}' requires a list, tuple, pybevy.array.Array, or NumPy-compatible array"
    ))
}

fn portable_dtype_for(dtype: ColumnDType) -> Option<ArrayDType> {
    Some(match dtype {
        ColumnDType::F32 => ArrayDType::Float32,
        ColumnDType::F64 => ArrayDType::Float64,
        ColumnDType::I32 => ArrayDType::Int32,
        ColumnDType::I64 => ArrayDType::Int64,
        ColumnDType::U32 => ArrayDType::Uint32,
        ColumnDType::U64 => return None,
        ColumnDType::Bool => ArrayDType::Bool,
    })
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
    /// 2D array (N, lanes) for composite (vector, quaternion, matrix) fields
    Lanes(PyReadonlyArray2<'py, f32>, usize),
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
            composite => ReadonlyArrayHolder::Lanes(
                array.extract::<PyReadonlyArray2<f32>>()?,
                composite.element_count(),
            ),
        };
        holders.push(holder);
    }

    let mut columns: Vec<(usize, FieldColumn<'_>)> =
        Vec::with_capacity(holders.len() + batch.field_values.len());
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
            ReadonlyArrayHolder::Lanes(array, lanes) => FieldColumn::Lanes(
                array.as_slice().map_err(|error| {
                    PyValueError::new_err(format!("Array not contiguous: {error}"))
                })?,
                *lanes,
            ),
        };
        columns.push((*field_idx, column));
    }
    for (field_idx, data) in &batch.field_values {
        let column =
            field_column_for(layout.fields[*field_idx].field_type, data).ok_or_else(|| {
                PyTypeError::new_err("custom batch column dtype does not match field")
            })?;
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
        let registration = batch.prepared_registration(py)?;
        let mut prepared = prepare_custom_batch(py, &batch)?;
        let component_id = register_prepared_custom_component(world, &registration);
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
