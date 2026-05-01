use std::ptr;

use bevy::mesh::{MeshVertexAttribute, VertexAttributeValues};
use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyAny};

#[pyclass(name = "MeshVertexAttribute", frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMeshVertexAttribute(pub MeshVertexAttribute);

impl From<MeshVertexAttribute> for PyMeshVertexAttribute {
    fn from(attribute: MeshVertexAttribute) -> Self {
        PyMeshVertexAttribute(attribute)
    }
}

#[pymethods]
impl PyMeshVertexAttribute {
    #[getter]
    pub fn name(&self) -> &'static str {
        self.0.name
    }

    #[getter]
    pub fn id(&self) -> String {
        format!("{:?}", self.0.id)
    }

    #[getter]
    pub fn format(&self) -> String {
        format!("{:?}", self.0.format)
    }

    fn __repr__(&self) -> String {
        format!(
            "MeshVertexAttribute(name=\"{}\", id={:?}, format={:?})",
            self.0.name, self.0.id, self.0.format
        )
    }
}

#[pyclass(name = "VertexAttributeValues", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyVertexAttributeValues(pub VertexAttributeValues);

#[pymethods]
impl PyVertexAttributeValues {
    #[new]
    pub fn new<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        if let Ok(vref) = obj.extract::<PyRef<PyVertexAttributeValues>>() {
            return Ok(vref.clone());
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

        Err(PyTypeError::new_err(
            "Unsupported array dtype/shape. Supported: float32 (n|n,2|n,3|n,4), float64 (n|n,2|n,3|n,4), uint32 (n), int32 (n)",
        ))
    }
}

impl From<VertexAttributeValues> for PyVertexAttributeValues {
    fn from(values: VertexAttributeValues) -> Self {
        PyVertexAttributeValues(values)
    }
}
