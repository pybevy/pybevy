use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    ptr,
    sync::{Mutex, OnceLock, PoisonError},
};

use bevy::mesh::{MeshVertexAttribute, MeshVertexAttributeId, VertexAttributeValues};
use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pybevy_array::{
    ArrayDType, ArrayError, ArrayStorage, DenseArrayCore, PyArray, Scalar,
    decode_read_only_le_bytes, owned_contiguous_le_bytes_from_any, read_only_storage,
};
use pybevy_core::{
    public_error,
    repr::{object_repr, str_repr},
};
use pybevy_macros::pyenum;
use pybevy_render::vertex_format::PyVertexFormat;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyBytes, PyList, PyTuple},
};

const UNSUPPORTED_ATTRIBUTE_ARRAY: &str = "Unsupported array dtype/shape. Supported: float32 (n|n,2|n,3|n,4), float64 (n|n,2|n,3|n,4), uint32 (n), int32 (n)";

fn bounded_vertex_attribute_values(
    core: &DenseArrayCore,
) -> Result<Option<VertexAttributeValues>, ArrayError> {
    let dtype = core.dtype();
    let shape = core.shape();
    if !matches!(
        (dtype, shape),
        (ArrayDType::Float32 | ArrayDType::Float64, [_])
            | (ArrayDType::Float32 | ArrayDType::Float64, [_, 2..=4])
            | (ArrayDType::Uint32 | ArrayDType::Int32, [_])
    ) {
        return Ok(None);
    }

    let values = core.to_scalars()?;
    let result = match (dtype, shape) {
        (ArrayDType::Float32 | ArrayDType::Float64, [_]) => {
            VertexAttributeValues::Float32(float_values(&values))
        }
        (ArrayDType::Float32 | ArrayDType::Float64, [_, 2]) => {
            VertexAttributeValues::Float32x2(float_rows(&values))
        }
        (ArrayDType::Float32 | ArrayDType::Float64, [_, 3]) => {
            VertexAttributeValues::Float32x3(float_rows(&values))
        }
        (ArrayDType::Float32 | ArrayDType::Float64, [_, 4]) => {
            VertexAttributeValues::Float32x4(float_rows(&values))
        }
        (ArrayDType::Uint32, [_]) => VertexAttributeValues::Uint32(
            values
                .into_iter()
                .map(|value| value.to_i64_trunc() as u32)
                .collect(),
        ),
        (ArrayDType::Int32, [_]) => VertexAttributeValues::Sint32(
            values
                .into_iter()
                .map(|value| value.to_i64_trunc() as i32)
                .collect(),
        ),
        _ => unreachable!("supported dtype and shape checked above"),
    };
    Ok(Some(result))
}

fn float_values(values: &[Scalar]) -> Vec<f32> {
    values.iter().map(|value| value.to_f64() as f32).collect()
}

fn float_rows<const N: usize>(values: &[Scalar]) -> Vec<[f32; N]> {
    values
        .chunks_exact(N)
        .map(|row| std::array::from_fn(|index| row[index].to_f64() as f32))
        .collect()
}

#[derive(Default)]
struct AttributeIdHasher(u64);

impl Hasher for AttributeIdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut value = [0_u8; 8];
        let count = bytes.len().min(value.len());
        value[..count].copy_from_slice(&bytes[..count]);
        self.0 = u64::from_ne_bytes(value);
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

pub(crate) fn attribute_id(attribute: &MeshVertexAttribute) -> u64 {
    // Bevy keeps MeshVertexAttributeId's u64 payload private. Its derived Hash
    // implementation writes that payload directly, so this recovers the public
    // attribute ID without depending on Debug formatting.
    let mut hasher = AttributeIdHasher::default();
    attribute.id.hash(&mut hasher);
    hasher.finish()
}

fn intern_attribute_name(name: &str) -> &'static str {
    static NAMES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let names = NAMES.get_or_init(|| Mutex::new(HashSet::new()));
    let mut names = names.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(existing) = names.get(name) {
        return existing;
    }
    let name = Box::leak(name.to_owned().into_boxed_str());
    names.insert(name);
    name
}

#[pyclass(
    from_py_object,
    name = "MeshVertexAttributeId",
    module = "pybevy.mesh",
    frozen,
    eq,
    hash
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyMeshVertexAttributeId(pub(crate) MeshVertexAttributeId);

impl PyMeshVertexAttributeId {
    fn raw(&self) -> u64 {
        let mut hasher = AttributeIdHasher::default();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

#[pymethods]
impl PyMeshVertexAttributeId {
    fn __repr__(&self) -> String {
        format!("MeshVertexAttributeId({})", self.raw())
    }
}

#[pyclass(
    name = "MeshVertexAttribute",
    module = "pybevy.mesh",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMeshVertexAttribute(pub MeshVertexAttribute);

impl From<MeshVertexAttribute> for PyMeshVertexAttribute {
    fn from(attribute: MeshVertexAttribute) -> Self {
        PyMeshVertexAttribute(attribute)
    }
}

#[pymethods]
impl PyMeshVertexAttribute {
    #[new]
    pub fn new(name: &str, id: u64, format: PyVertexFormat) -> Self {
        let name = intern_attribute_name(name);
        PyMeshVertexAttribute(MeshVertexAttribute::new(name, id, format.into()))
    }

    #[getter]
    pub fn name(&self) -> &'static str {
        self.0.name
    }

    #[getter]
    pub fn id(&self, py: Python<'_>) -> PyResult<Py<PyMeshVertexAttributeId>> {
        Py::new(py, PyMeshVertexAttributeId(self.0.id))
    }

    #[getter]
    pub fn format(&self) -> PyVertexFormat {
        self.0.format.into()
    }

    fn __repr__(&self) -> String {
        format!(
            "MeshVertexAttribute(name={}, id={:?}, format={})",
            str_repr(self.0.name),
            attribute_id(&self.0),
            object_repr(PyVertexFormat::from(self.0.format))
        )
    }
}

#[pyenum(VertexAttributeValues, manual)]
#[pyclass(
    name = "VertexAttributeValues",
    module = "pybevy.mesh",
    eq,
    frozen,
    subclass,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyVertexAttributeValues(pub VertexAttributeValues);

impl PyVertexAttributeValues {
    pub fn new<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        if let Ok(vref) = obj.extract::<PyRef<PyVertexAttributeValues>>() {
            return Ok(vref.clone());
        }

        // Convert sequences without requiring the optional NumPy package.
        if obj.is_instance_of::<PyList>() || obj.is_instance_of::<PyTuple>() {
            return sequence_attribute_values(obj).map(Self);
        }

        if let Ok(array) = obj.extract::<PyRef<PyArray>>() {
            let values = bounded_vertex_attribute_values(&array.core)
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?
                .ok_or_else(|| PyTypeError::new_err(UNSUPPORTED_ATTRIBUTE_ARRAY))?;
            return Ok(PyVertexAttributeValues(values));
        }

        if obj.py().import("numpy").is_err() {
            return sequence_attribute_values(obj).map(Self);
        }

        if let Ok(arr) = obj.extract::<PyReadonlyArray1<f32>>() {
            let slice = arr.as_slice()?;
            return Ok(PyVertexAttributeValues(VertexAttributeValues::Float32(
                slice.to_vec(),
            )));
        }
        if let Ok(arr) = obj.extract::<PyReadonlyArray1<u32>>() {
            let slice = arr.as_slice()?;
            return Ok(PyVertexAttributeValues(VertexAttributeValues::Uint32(
                slice.to_vec(),
            )));
        }
        if let Ok(arr) = obj.extract::<PyReadonlyArray1<i32>>() {
            let slice = arr.as_slice()?;
            return Ok(PyVertexAttributeValues(VertexAttributeValues::Sint32(
                slice.to_vec(),
            )));
        }

        if let Ok(arr) = obj.extract::<PyReadonlyArray2<f32>>() {
            let shape = arr.shape();
            if shape.len() == 2 && arr.as_array().is_standard_layout() {
                let n = shape[0];
                match shape[1] {
                    2 => {
                        let flat = arr.as_slice()?;
                        // SAFETY: copy_nonoverlapping is valid because:
                        // - flat has n*2 f32s (verified by shape check)
                        // - out has capacity for n [f32; 2] = n*2 f32s
                        // - [f32; 2] and [f32] have same alignment
                        let out: Vec<[f32; 2]> = unsafe {
                            let mut out = Vec::with_capacity(n);
                            ptr::copy_nonoverlapping(
                                flat.as_ptr(),
                                out.as_mut_ptr() as *mut f32,
                                n * 2,
                            );
                            out.set_len(n);
                            out
                        };
                        return Ok(PyVertexAttributeValues(VertexAttributeValues::Float32x2(
                            out,
                        )));
                    }
                    3 => {
                        let flat = arr.as_slice()?;
                        // SAFETY: copy_nonoverlapping is valid because:
                        // - flat has n*3 f32s (verified by shape check)
                        // - out has capacity for n [f32; 3] = n*3 f32s
                        // - [f32; 3] and [f32] have same alignment
                        let out: Vec<[f32; 3]> = unsafe {
                            let mut out = Vec::with_capacity(n);
                            ptr::copy_nonoverlapping(
                                flat.as_ptr(),
                                out.as_mut_ptr() as *mut f32,
                                n * 3,
                            );
                            out.set_len(n);
                            out
                        };
                        return Ok(PyVertexAttributeValues(VertexAttributeValues::Float32x3(
                            out,
                        )));
                    }
                    4 => {
                        let flat = arr.as_slice()?;
                        // SAFETY: copy_nonoverlapping is valid because:
                        // - flat has n*4 f32s (verified by shape check)
                        // - out has capacity for n [f32; 4] = n*4 f32s
                        // - [f32; 4] and [f32] have same alignment
                        let out: Vec<[f32; 4]> = unsafe {
                            let mut out = Vec::with_capacity(n);
                            ptr::copy_nonoverlapping(
                                flat.as_ptr(),
                                out.as_mut_ptr() as *mut f32,
                                n * 4,
                            );
                            out.set_len(n);
                            out
                        };
                        return Ok(PyVertexAttributeValues(VertexAttributeValues::Float32x4(
                            out,
                        )));
                    }
                    _ => {}
                }
            }
        }

        if let Ok(arr) = obj.extract::<PyReadonlyArray1<f64>>() {
            let slice = arr.as_slice()?;
            let v: Vec<f32> = slice.iter().map(|&x| x as f32).collect();
            return Ok(PyVertexAttributeValues(VertexAttributeValues::Float32(v)));
        }
        if let Ok(arr) = obj.extract::<PyReadonlyArray2<f64>>() {
            let shape = arr.shape();
            if shape.len() == 2 && arr.as_array().is_standard_layout() {
                let n = shape[0];
                match shape[1] {
                    2 => {
                        let flat = arr.as_slice()?;
                        let mut out: Vec<[f32; 2]> = Vec::with_capacity(n);
                        out.extend(flat.chunks_exact(2).map(|c| [c[0] as f32, c[1] as f32]));
                        return Ok(PyVertexAttributeValues(VertexAttributeValues::Float32x2(
                            out,
                        )));
                    }
                    3 => {
                        let flat = arr.as_slice()?;
                        let mut out: Vec<[f32; 3]> = Vec::with_capacity(n);
                        out.extend(
                            flat.chunks_exact(3)
                                .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]),
                        );
                        return Ok(PyVertexAttributeValues(VertexAttributeValues::Float32x3(
                            out,
                        )));
                    }
                    4 => {
                        let flat = arr.as_slice()?;
                        let mut out: Vec<[f32; 4]> = Vec::with_capacity(n);
                        out.extend(
                            flat.chunks_exact(4)
                                .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32]),
                        );
                        return Ok(PyVertexAttributeValues(VertexAttributeValues::Float32x4(
                            out,
                        )));
                    }
                    _ => {}
                }
            }
        }

        if is_non_contiguous_array(obj) {
            return Err(PyTypeError::new_err(
                public_error::NON_CONTIGUOUS_ATTRIBUTE_ARRAY,
            ));
        }
        // An array with an unsupported dtype must keep naming the dtype.
        if obj.hasattr("dtype").unwrap_or(false) || obj.is_instance_of::<PyBytes>() {
            return Err(PyTypeError::new_err(UNSUPPORTED_ATTRIBUTE_ARRAY));
        }
        sequence_attribute_values(obj).map(Self)
    }
}

#[pymethods]
impl PyVertexAttributeValues {
    fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        materialize_vertex_attribute_values(py, self.0.clone())
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<Self>> {
        self.__copy__(py)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn __len__(&self) -> usize {
        self.0.len()
    }

    fn __repr__(&self) -> String {
        format!("VertexAttributeValues.{:?}", self.0)
    }
}

fn sequence_attribute_values(obj: &Bound<'_, PyAny>) -> PyResult<VertexAttributeValues> {
    if let Ok(values) = obj.extract::<Vec<f32>>() {
        return Ok(VertexAttributeValues::Float32(values));
    }
    let rows = obj
        .extract::<Vec<Vec<f32>>>()
        .map_err(|_| PyTypeError::new_err(UNSUPPORTED_ATTRIBUTE_ARRAY))?;
    let columns = rows.first().map_or(0, Vec::len);
    if !(2..=4).contains(&columns) || rows.iter().any(|row| row.len() != columns) {
        return Err(PyTypeError::new_err(UNSUPPORTED_ATTRIBUTE_ARRAY));
    }
    let flat: Vec<Scalar> = rows
        .into_iter()
        .flatten()
        .map(|value| Scalar::F64(value as f64))
        .collect();
    Ok(match columns {
        2 => VertexAttributeValues::Float32x2(float_rows(&flat)),
        3 => VertexAttributeValues::Float32x3(float_rows(&flat)),
        4 => VertexAttributeValues::Float32x4(float_rows(&flat)),
        _ => unreachable!("validated row width"),
    })
}

/// True for a numpy array whose dtype and shape would be fine but whose layout is not.
fn is_non_contiguous_array(obj: &Bound<'_, PyAny>) -> bool {
    let Ok(flags) = obj.getattr("flags") else {
        return false;
    };
    matches!(
        flags
            .getattr("c_contiguous")
            .and_then(|v| v.extract::<bool>()),
        Ok(false)
    )
}

impl From<VertexAttributeValues> for PyVertexAttributeValues {
    fn from(values: VertexAttributeValues) -> Self {
        PyVertexAttributeValues(values)
    }
}

trait VavElement: Copy {
    fn from_scalar(value: Scalar) -> PyResult<Self>;
    fn storage(values: Vec<Self>) -> ArrayStorage;
}

macro_rules! impl_integer_element {
    ($ty:ty, $storage:ident) => {
        impl VavElement for $ty {
            fn from_scalar(value: Scalar) -> PyResult<Self> {
                let Scalar::I64(value) = value else {
                    return Err(PyTypeError::new_err(
                        "integer vertex attribute values require integers",
                    ));
                };
                Self::try_from(value).map_err(|_| {
                    PyValueError::new_err(format!(
                        "vertex attribute value {value} is out of range for {}",
                        stringify!($ty)
                    ))
                })
            }

            fn storage(values: Vec<Self>) -> ArrayStorage {
                ArrayStorage::$storage(values)
            }
        }
    };
}

impl_integer_element!(u8, Uint8);
impl_integer_element!(i8, Int8);
impl_integer_element!(u16, Uint16);
impl_integer_element!(i16, Int16);
impl_integer_element!(u32, Uint32);
impl_integer_element!(i32, Int32);

impl VavElement for f32 {
    fn from_scalar(value: Scalar) -> PyResult<Self> {
        if matches!(value, Scalar::Bool(_)) {
            return Err(PyTypeError::new_err(
                "float vertex attribute values require numbers",
            ));
        }
        Ok(value.to_f64() as f32)
    }

    fn storage(values: Vec<Self>) -> ArrayStorage {
        ArrayStorage::Float32(values)
    }
}

impl VavElement for f64 {
    fn from_scalar(value: Scalar) -> PyResult<Self> {
        if matches!(value, Scalar::Bool(_)) {
            return Err(PyTypeError::new_err(
                "float vertex attribute values require numbers",
            ));
        }
        Ok(value.to_f64())
    }

    fn storage(values: Vec<Self>) -> ArrayStorage {
        ArrayStorage::Float64(values)
    }
}

impl VavElement for half::f16 {
    fn from_scalar(value: Scalar) -> PyResult<Self> {
        if matches!(value, Scalar::Bool(_)) {
            return Err(PyTypeError::new_err(
                "float vertex attribute values require numbers",
            ));
        }
        Ok(half::f16::from_f64(value.to_f64()))
    }

    fn storage(values: Vec<Self>) -> ArrayStorage {
        ArrayStorage::Float16(values.into_iter().map(half::f16::to_bits).collect())
    }
}

fn variant_scalars<T: VavElement, const N: usize>(value: &Bound<'_, PyAny>) -> PyResult<Vec<T>> {
    let (dtype, shape, bytes) = owned_contiguous_le_bytes_from_any(value, None)?;
    let valid_shape = if N == 1 {
        shape.len() == 1
    } else {
        shape.as_slice() == [0] || matches!(shape.as_slice(), [_, columns] if *columns == N)
    };
    if !valid_shape {
        return Err(PyValueError::new_err(format!(
            "vertex attribute values require shape (N,{N})"
        )));
    }
    let core = decode_read_only_le_bytes(dtype, &bytes, &shape)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    core.to_scalars()
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?
        .into_iter()
        .map(T::from_scalar)
        .collect()
}

fn variant_rows<T: VavElement, const N: usize>(value: &Bound<'_, PyAny>) -> PyResult<Vec<[T; N]>> {
    let values = variant_scalars::<T, N>(value)?;
    Ok(values
        .chunks_exact(N)
        .map(|row| std::array::from_fn(|i| row[i]))
        .collect())
}

macro_rules! scalar_variant {
    ($class:ident, $variant:ident, $name:literal, $ty:ty) => {
        #[pyclass(name = $name, module = "pybevy.mesh", extends = PyVertexAttributeValues, frozen)]
        pub struct $class;

        #[pymethods]
        #[allow(non_upper_case_globals)]
        impl $class {
            #[classattr]
            const __qualname__: &'static str = concat!("VertexAttributeValues.", $name);

            #[classattr]
            fn __match_args__() -> (&'static str,) {
                ("value",)
            }

            #[new]
            fn new(value: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
                let values = variant_scalars::<$ty, 1>(value)?;
                Ok(PyClassInitializer::from(PyVertexAttributeValues(
                    VertexAttributeValues::$variant(values),
                ))
                .add_subclass(Self))
            }

            #[getter]
            fn value(slf: PyRef<'_, Self>) -> PyResult<PyArray> {
                let base = slf.into_super();
                let VertexAttributeValues::$variant(values) = &base.0 else {
                    unreachable!("frozen vertex attribute changed variant")
                };
                read_only_storage(
                    <$ty as VavElement>::storage(values.clone()),
                    &[values.len()],
                )
            }
        }
    };
}

macro_rules! vector_variant {
    ($class:ident, $variant:ident, $name:literal, $ty:ty, $width:literal) => {
        #[pyclass(name = $name, module = "pybevy.mesh", extends = PyVertexAttributeValues, frozen)]
        pub struct $class;

        #[pymethods]
        #[allow(non_upper_case_globals)]
        impl $class {
            #[classattr]
            const __qualname__: &'static str = concat!("VertexAttributeValues.", $name);

            #[classattr]
            fn __match_args__() -> (&'static str,) {
                ("value",)
            }

            #[new]
            fn new(value: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
                let values = variant_rows::<$ty, $width>(value)?;
                Ok(PyClassInitializer::from(PyVertexAttributeValues(
                    VertexAttributeValues::$variant(values),
                ))
                .add_subclass(Self))
            }

            #[getter]
            fn value(slf: PyRef<'_, Self>) -> PyResult<PyArray> {
                let base = slf.into_super();
                let VertexAttributeValues::$variant(values) = &base.0 else {
                    unreachable!("frozen vertex attribute changed variant")
                };
                let flat = values.iter().flat_map(|row| row.iter().copied()).collect();
                read_only_storage(<$ty as VavElement>::storage(flat), &[values.len(), $width])
            }
        }
    };
}

// Exact, owned Bevy vertex-format variants.
scalar_variant!(PyVertexAttributeValuesUint8, Uint8, "Uint8", u8);
vector_variant!(PyVertexAttributeValuesUint8x2, Uint8x2, "Uint8x2", u8, 2);
vector_variant!(PyVertexAttributeValuesUint8x4, Uint8x4, "Uint8x4", u8, 4);
scalar_variant!(PyVertexAttributeValuesSint8, Sint8, "Sint8", i8);
vector_variant!(PyVertexAttributeValuesSint8x2, Sint8x2, "Sint8x2", i8, 2);
vector_variant!(PyVertexAttributeValuesSint8x4, Sint8x4, "Sint8x4", i8, 4);
scalar_variant!(PyVertexAttributeValuesUnorm8, Unorm8, "Unorm8", u8);
vector_variant!(PyVertexAttributeValuesUnorm8x2, Unorm8x2, "Unorm8x2", u8, 2);
vector_variant!(PyVertexAttributeValuesUnorm8x4, Unorm8x4, "Unorm8x4", u8, 4);
scalar_variant!(PyVertexAttributeValuesSnorm8, Snorm8, "Snorm8", i8);
vector_variant!(PyVertexAttributeValuesSnorm8x2, Snorm8x2, "Snorm8x2", i8, 2);
vector_variant!(PyVertexAttributeValuesSnorm8x4, Snorm8x4, "Snorm8x4", i8, 4);
scalar_variant!(PyVertexAttributeValuesUint16, Uint16, "Uint16", u16);
vector_variant!(
    PyVertexAttributeValuesUint16x2,
    Uint16x2,
    "Uint16x2",
    u16,
    2
);
vector_variant!(
    PyVertexAttributeValuesUint16x4,
    Uint16x4,
    "Uint16x4",
    u16,
    4
);
scalar_variant!(PyVertexAttributeValuesSint16, Sint16, "Sint16", i16);
vector_variant!(
    PyVertexAttributeValuesSint16x2,
    Sint16x2,
    "Sint16x2",
    i16,
    2
);
vector_variant!(
    PyVertexAttributeValuesSint16x4,
    Sint16x4,
    "Sint16x4",
    i16,
    4
);
scalar_variant!(PyVertexAttributeValuesUnorm16, Unorm16, "Unorm16", u16);
vector_variant!(
    PyVertexAttributeValuesUnorm16x2,
    Unorm16x2,
    "Unorm16x2",
    u16,
    2
);
vector_variant!(
    PyVertexAttributeValuesUnorm16x4,
    Unorm16x4,
    "Unorm16x4",
    u16,
    4
);
scalar_variant!(PyVertexAttributeValuesSnorm16, Snorm16, "Snorm16", i16);
vector_variant!(
    PyVertexAttributeValuesSnorm16x2,
    Snorm16x2,
    "Snorm16x2",
    i16,
    2
);
vector_variant!(
    PyVertexAttributeValuesSnorm16x4,
    Snorm16x4,
    "Snorm16x4",
    i16,
    4
);
scalar_variant!(
    PyVertexAttributeValuesFloat16,
    Float16,
    "Float16",
    half::f16
);
vector_variant!(
    PyVertexAttributeValuesFloat16x2,
    Float16x2,
    "Float16x2",
    half::f16,
    2
);
vector_variant!(
    PyVertexAttributeValuesFloat16x4,
    Float16x4,
    "Float16x4",
    half::f16,
    4
);
scalar_variant!(PyVertexAttributeValuesFloat32, Float32, "Float32", f32);
vector_variant!(
    PyVertexAttributeValuesFloat32x2,
    Float32x2,
    "Float32x2",
    f32,
    2
);
vector_variant!(
    PyVertexAttributeValuesFloat32x3,
    Float32x3,
    "Float32x3",
    f32,
    3
);
vector_variant!(
    PyVertexAttributeValuesFloat32x4,
    Float32x4,
    "Float32x4",
    f32,
    4
);
scalar_variant!(PyVertexAttributeValuesUint32, Uint32, "Uint32", u32);
vector_variant!(
    PyVertexAttributeValuesUint32x2,
    Uint32x2,
    "Uint32x2",
    u32,
    2
);
vector_variant!(
    PyVertexAttributeValuesUint32x3,
    Uint32x3,
    "Uint32x3",
    u32,
    3
);
vector_variant!(
    PyVertexAttributeValuesUint32x4,
    Uint32x4,
    "Uint32x4",
    u32,
    4
);
scalar_variant!(PyVertexAttributeValuesSint32, Sint32, "Sint32", i32);
vector_variant!(
    PyVertexAttributeValuesSint32x2,
    Sint32x2,
    "Sint32x2",
    i32,
    2
);
vector_variant!(
    PyVertexAttributeValuesSint32x3,
    Sint32x3,
    "Sint32x3",
    i32,
    3
);
vector_variant!(
    PyVertexAttributeValuesSint32x4,
    Sint32x4,
    "Sint32x4",
    i32,
    4
);
scalar_variant!(PyVertexAttributeValuesFloat64, Float64, "Float64", f64);
vector_variant!(
    PyVertexAttributeValuesFloat64x2,
    Float64x2,
    "Float64x2",
    f64,
    2
);
vector_variant!(
    PyVertexAttributeValuesFloat64x3,
    Float64x3,
    "Float64x3",
    f64,
    3
);
vector_variant!(
    PyVertexAttributeValuesFloat64x4,
    Float64x4,
    "Float64x4",
    f64,
    4
);
scalar_variant!(
    PyVertexAttributeValuesUnorm10_10_10_2,
    Unorm10_10_10_2,
    "Unorm10_10_10_2",
    u32
);
vector_variant!(
    PyVertexAttributeValuesUnorm8x4Bgra,
    Unorm8x4Bgra,
    "Unorm8x4Bgra",
    u8,
    4
);

pub fn materialize_vertex_attribute_values(
    py: Python<'_>,
    value: VertexAttributeValues,
) -> PyResult<Py<PyVertexAttributeValues>> {
    macro_rules! materialize {
        ($class:ident) => {{
            let instance = Py::new(
                py,
                PyClassInitializer::from(PyVertexAttributeValues(value)).add_subclass($class),
            )?;
            Ok(instance.into_bound(py).into_super().unbind())
        }};
    }
    match &value {
        VertexAttributeValues::Uint8(_) => materialize!(PyVertexAttributeValuesUint8),
        VertexAttributeValues::Uint8x2(_) => materialize!(PyVertexAttributeValuesUint8x2),
        VertexAttributeValues::Uint8x4(_) => materialize!(PyVertexAttributeValuesUint8x4),
        VertexAttributeValues::Sint8(_) => materialize!(PyVertexAttributeValuesSint8),
        VertexAttributeValues::Sint8x2(_) => materialize!(PyVertexAttributeValuesSint8x2),
        VertexAttributeValues::Sint8x4(_) => materialize!(PyVertexAttributeValuesSint8x4),
        VertexAttributeValues::Unorm8(_) => materialize!(PyVertexAttributeValuesUnorm8),
        VertexAttributeValues::Unorm8x2(_) => materialize!(PyVertexAttributeValuesUnorm8x2),
        VertexAttributeValues::Unorm8x4(_) => materialize!(PyVertexAttributeValuesUnorm8x4),
        VertexAttributeValues::Snorm8(_) => materialize!(PyVertexAttributeValuesSnorm8),
        VertexAttributeValues::Snorm8x2(_) => materialize!(PyVertexAttributeValuesSnorm8x2),
        VertexAttributeValues::Snorm8x4(_) => materialize!(PyVertexAttributeValuesSnorm8x4),
        VertexAttributeValues::Uint16(_) => materialize!(PyVertexAttributeValuesUint16),
        VertexAttributeValues::Uint16x2(_) => materialize!(PyVertexAttributeValuesUint16x2),
        VertexAttributeValues::Uint16x4(_) => materialize!(PyVertexAttributeValuesUint16x4),
        VertexAttributeValues::Sint16(_) => materialize!(PyVertexAttributeValuesSint16),
        VertexAttributeValues::Sint16x2(_) => materialize!(PyVertexAttributeValuesSint16x2),
        VertexAttributeValues::Sint16x4(_) => materialize!(PyVertexAttributeValuesSint16x4),
        VertexAttributeValues::Unorm16(_) => materialize!(PyVertexAttributeValuesUnorm16),
        VertexAttributeValues::Unorm16x2(_) => materialize!(PyVertexAttributeValuesUnorm16x2),
        VertexAttributeValues::Unorm16x4(_) => materialize!(PyVertexAttributeValuesUnorm16x4),
        VertexAttributeValues::Snorm16(_) => materialize!(PyVertexAttributeValuesSnorm16),
        VertexAttributeValues::Snorm16x2(_) => materialize!(PyVertexAttributeValuesSnorm16x2),
        VertexAttributeValues::Snorm16x4(_) => materialize!(PyVertexAttributeValuesSnorm16x4),
        VertexAttributeValues::Float16(_) => materialize!(PyVertexAttributeValuesFloat16),
        VertexAttributeValues::Float16x2(_) => materialize!(PyVertexAttributeValuesFloat16x2),
        VertexAttributeValues::Float16x4(_) => materialize!(PyVertexAttributeValuesFloat16x4),
        VertexAttributeValues::Float32(_) => materialize!(PyVertexAttributeValuesFloat32),
        VertexAttributeValues::Float32x2(_) => materialize!(PyVertexAttributeValuesFloat32x2),
        VertexAttributeValues::Float32x3(_) => materialize!(PyVertexAttributeValuesFloat32x3),
        VertexAttributeValues::Float32x4(_) => materialize!(PyVertexAttributeValuesFloat32x4),
        VertexAttributeValues::Uint32(_) => materialize!(PyVertexAttributeValuesUint32),
        VertexAttributeValues::Uint32x2(_) => materialize!(PyVertexAttributeValuesUint32x2),
        VertexAttributeValues::Uint32x3(_) => materialize!(PyVertexAttributeValuesUint32x3),
        VertexAttributeValues::Uint32x4(_) => materialize!(PyVertexAttributeValuesUint32x4),
        VertexAttributeValues::Sint32(_) => materialize!(PyVertexAttributeValuesSint32),
        VertexAttributeValues::Sint32x2(_) => materialize!(PyVertexAttributeValuesSint32x2),
        VertexAttributeValues::Sint32x3(_) => materialize!(PyVertexAttributeValuesSint32x3),
        VertexAttributeValues::Sint32x4(_) => materialize!(PyVertexAttributeValuesSint32x4),
        VertexAttributeValues::Float64(_) => materialize!(PyVertexAttributeValuesFloat64),
        VertexAttributeValues::Float64x2(_) => materialize!(PyVertexAttributeValuesFloat64x2),
        VertexAttributeValues::Float64x3(_) => materialize!(PyVertexAttributeValuesFloat64x3),
        VertexAttributeValues::Float64x4(_) => materialize!(PyVertexAttributeValuesFloat64x4),
        VertexAttributeValues::Unorm10_10_10_2(_) => {
            materialize!(PyVertexAttributeValuesUnorm10_10_10_2)
        }
        VertexAttributeValues::Unorm8x4Bgra(_) => materialize!(PyVertexAttributeValuesUnorm8x4Bgra),
    }
}

pub fn register_vertex_attribute_values_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("VertexAttributeValues")?;
    base.setattr("Uint8", py.get_type::<PyVertexAttributeValuesUint8>())?;
    base.setattr("Uint8x2", py.get_type::<PyVertexAttributeValuesUint8x2>())?;
    base.setattr("Uint8x4", py.get_type::<PyVertexAttributeValuesUint8x4>())?;
    base.setattr("Sint8", py.get_type::<PyVertexAttributeValuesSint8>())?;
    base.setattr("Sint8x2", py.get_type::<PyVertexAttributeValuesSint8x2>())?;
    base.setattr("Sint8x4", py.get_type::<PyVertexAttributeValuesSint8x4>())?;
    base.setattr("Unorm8", py.get_type::<PyVertexAttributeValuesUnorm8>())?;
    base.setattr("Unorm8x2", py.get_type::<PyVertexAttributeValuesUnorm8x2>())?;
    base.setattr("Unorm8x4", py.get_type::<PyVertexAttributeValuesUnorm8x4>())?;
    base.setattr("Snorm8", py.get_type::<PyVertexAttributeValuesSnorm8>())?;
    base.setattr("Snorm8x2", py.get_type::<PyVertexAttributeValuesSnorm8x2>())?;
    base.setattr("Snorm8x4", py.get_type::<PyVertexAttributeValuesSnorm8x4>())?;
    base.setattr("Uint16", py.get_type::<PyVertexAttributeValuesUint16>())?;
    base.setattr("Uint16x2", py.get_type::<PyVertexAttributeValuesUint16x2>())?;
    base.setattr("Uint16x4", py.get_type::<PyVertexAttributeValuesUint16x4>())?;
    base.setattr("Sint16", py.get_type::<PyVertexAttributeValuesSint16>())?;
    base.setattr("Sint16x2", py.get_type::<PyVertexAttributeValuesSint16x2>())?;
    base.setattr("Sint16x4", py.get_type::<PyVertexAttributeValuesSint16x4>())?;
    base.setattr("Unorm16", py.get_type::<PyVertexAttributeValuesUnorm16>())?;
    base.setattr(
        "Unorm16x2",
        py.get_type::<PyVertexAttributeValuesUnorm16x2>(),
    )?;
    base.setattr(
        "Unorm16x4",
        py.get_type::<PyVertexAttributeValuesUnorm16x4>(),
    )?;
    base.setattr("Snorm16", py.get_type::<PyVertexAttributeValuesSnorm16>())?;
    base.setattr(
        "Snorm16x2",
        py.get_type::<PyVertexAttributeValuesSnorm16x2>(),
    )?;
    base.setattr(
        "Snorm16x4",
        py.get_type::<PyVertexAttributeValuesSnorm16x4>(),
    )?;
    base.setattr("Float16", py.get_type::<PyVertexAttributeValuesFloat16>())?;
    base.setattr(
        "Float16x2",
        py.get_type::<PyVertexAttributeValuesFloat16x2>(),
    )?;
    base.setattr(
        "Float16x4",
        py.get_type::<PyVertexAttributeValuesFloat16x4>(),
    )?;
    base.setattr("Float32", py.get_type::<PyVertexAttributeValuesFloat32>())?;
    base.setattr(
        "Float32x2",
        py.get_type::<PyVertexAttributeValuesFloat32x2>(),
    )?;
    base.setattr(
        "Float32x3",
        py.get_type::<PyVertexAttributeValuesFloat32x3>(),
    )?;
    base.setattr(
        "Float32x4",
        py.get_type::<PyVertexAttributeValuesFloat32x4>(),
    )?;
    base.setattr("Uint32", py.get_type::<PyVertexAttributeValuesUint32>())?;
    base.setattr("Uint32x2", py.get_type::<PyVertexAttributeValuesUint32x2>())?;
    base.setattr("Uint32x3", py.get_type::<PyVertexAttributeValuesUint32x3>())?;
    base.setattr("Uint32x4", py.get_type::<PyVertexAttributeValuesUint32x4>())?;
    base.setattr("Sint32", py.get_type::<PyVertexAttributeValuesSint32>())?;
    base.setattr("Sint32x2", py.get_type::<PyVertexAttributeValuesSint32x2>())?;
    base.setattr("Sint32x3", py.get_type::<PyVertexAttributeValuesSint32x3>())?;
    base.setattr("Sint32x4", py.get_type::<PyVertexAttributeValuesSint32x4>())?;
    base.setattr("Float64", py.get_type::<PyVertexAttributeValuesFloat64>())?;
    base.setattr(
        "Float64x2",
        py.get_type::<PyVertexAttributeValuesFloat64x2>(),
    )?;
    base.setattr(
        "Float64x3",
        py.get_type::<PyVertexAttributeValuesFloat64x3>(),
    )?;
    base.setattr(
        "Float64x4",
        py.get_type::<PyVertexAttributeValuesFloat64x4>(),
    )?;
    base.setattr(
        "Unorm10_10_10_2",
        py.get_type::<PyVertexAttributeValuesUnorm10_10_10_2>(),
    )?;
    base.setattr(
        "Unorm8x4Bgra",
        py.get_type::<PyVertexAttributeValuesUnorm8x4Bgra>(),
    )?;
    Ok(())
}
