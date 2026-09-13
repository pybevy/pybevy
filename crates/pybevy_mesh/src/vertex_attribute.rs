use std::{
    hash::{Hash, Hasher},
    ptr,
};

use bevy::mesh::{MeshVertexAttribute, MeshVertexAttributeId, VertexAttributeValues};
use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pybevy_array::{ArrayDType, ArrayError, DenseArrayCore, PyArray, Scalar};
use pybevy_core::{PyComponent, public_error};
use pybevy_macros::pywrap;
use pybevy_render::vertex_format::PyVertexFormat;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
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

#[pywrap(MeshVertexAttributeId, copy)]
#[pyclass(
    from_py_object,
    name = "MeshVertexAttributeId",
    module = "pybevy.mesh",
    extends = PyComponent,
    frozen,
    eq,
    hash
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyMeshVertexAttributeId(pub(crate) MeshVertexAttributeId);

#[pymethods]
impl PyMeshVertexAttributeId {
    #[getter]
    pub fn value(&self) -> u64 {
        let mut hasher = AttributeIdHasher::default();
        self.0.hash(&mut hasher);
        hasher.finish()
    }

    fn __repr__(&self) -> String {
        format!("MeshVertexAttributeId({})", self.value())
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
        let boxed: Box<str> = name.into();
        let name: &'static str = Box::leak(boxed);
        PyMeshVertexAttribute(MeshVertexAttribute::new(name, id, format.into()))
    }

    #[getter]
    pub fn name(&self) -> &'static str {
        self.0.name
    }

    #[getter]
    pub fn id(&self, py: Python<'_>) -> PyResult<Py<PyMeshVertexAttributeId>> {
        Py::new(py, (PyMeshVertexAttributeId(self.0.id), PyComponent))
    }

    #[getter]
    pub fn format(&self) -> PyVertexFormat {
        self.0.format.into()
    }

    fn __repr__(&self) -> String {
        format!(
            "MeshVertexAttribute(name=\"{}\", id={:?}, format={:?})",
            self.0.name, self.0.id, self.0.format
        )
    }
}

#[pyclass(
    name = "VertexAttributeValues",
    module = "pybevy.mesh",
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyVertexAttributeValues(pub VertexAttributeValues);

#[pymethods]
impl PyVertexAttributeValues {
    #[new]
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
