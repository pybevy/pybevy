use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, Mesh, VertexAttributeValues},
};
use numpy::{
    PyArray2, PyArrayMethods, PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods,
    ndarray::{ArrayView2, ArrayViewMut2},
};
use pybevy_core::{AssetInputConverter, AssetStorage, PyAsset};
use pybevy_image::image::PyRenderAssetUsages;
use pybevy_macros::pyasset;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
};

use crate::{
    indices::PyIndices,
    mesh_builder::PyMeshBuilder,
    meshable::{PyMeshable, meshable_to_mesh},
    primitive_topology::PyPrimitiveTopology,
    vertex_attribute::{PyMeshVertexAttribute, PyVertexAttributeValues},
};
#[pyasset(Mesh, bridge, input_converter)]
#[pyclass(name = "Mesh", extends = PyAsset)]
#[derive(Debug)]
pub struct PyMesh {
    pub(crate) storage: AssetStorage<Mesh>,
}

impl AssetInputConverter for PyMesh {
    fn try_convert_input<'py>(
        asset: &Bound<'py, PyAny>,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        if asset.is_instance_of::<PyMeshBuilder>() {
            return Ok(Some(asset.call_method0("build")?));
        }
        if asset.is_instance_of::<PyMeshable>() {
            // Wrap in a fresh PyMesh so bridge.add() can uniformly extract+take.
            let mesh = meshable_to_mesh(asset)?;
            let py_mesh = Py::new(py, PyMesh::from_owned(mesh))?;
            return Ok(Some(py_mesh.into_bound(py).into_any()));
        }
        Ok(None)
    }
}

macro_rules! mesh_with {
    ($s:expr, $f:expr) => {{ $f((*$s).as_ref()?) }};
}

macro_rules! mesh_with_mut {
    ($s:expr, $f:expr) => {{ $f((*$s).as_mut()?) }};
}

#[pymethods]
impl PyMesh {
    #[classattr]
    #[allow(non_snake_case)]
    pub fn ATTRIBUTE_POSITION(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_POSITION))
    }

    #[classattr]
    #[allow(non_snake_case)]
    pub fn ATTRIBUTE_NORMAL(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_NORMAL))
    }

    #[classattr]
    #[allow(non_snake_case)]
    pub fn ATTRIBUTE_UV_0(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_UV_0))
    }

    #[classattr]
    #[allow(non_snake_case)]
    pub fn ATTRIBUTE_UV_1(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_UV_1))
    }

    #[classattr]
    #[allow(non_snake_case)]
    pub fn ATTRIBUTE_TANGENT(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_TANGENT))
    }

    #[classattr]
    #[allow(non_snake_case)]
    pub fn ATTRIBUTE_COLOR(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_COLOR))
    }

    #[classattr]
    #[allow(non_snake_case)]
    pub fn ATTRIBUTE_JOINT_WEIGHT(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(
            py,
            PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_JOINT_WEIGHT),
        )
    }

    #[classattr]
    #[allow(non_snake_case)]
    pub fn ATTRIBUTE_JOINT_INDEX(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_JOINT_INDEX))
    }

    #[new]
    pub fn new(primitive_topology: PyPrimitiveTopology) -> (Self, PyAsset) {
        Self::from_owned(Mesh::new(
            primitive_topology.into(),
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        ))
    }

    pub fn primitive_topology(&self) -> PyResult<PyPrimitiveTopology> {
        Ok(mesh_with!(self, |mesh: &Mesh| mesh
            .primitive_topology()
            .into()))
    }

    #[getter]
    pub fn asset_usage(&self) -> PyResult<PyRenderAssetUsages> {
        Ok(mesh_with!(self, |mesh: &Mesh| mesh.asset_usage.into()))
    }

    #[getter]
    pub fn enable_raytracing(&self) -> PyResult<bool> {
        Ok(mesh_with!(self, |mesh: &Mesh| mesh.enable_raytracing))
    }

    pub fn get_vertex_buffer_size(&self) -> PyResult<usize> {
        Ok(mesh_with!(self, |mesh: &Mesh| mesh.get_vertex_buffer_size()))
    }

    pub fn count_vertices(&self) -> PyResult<usize> {
        Ok(mesh_with!(self, |mesh: &Mesh| mesh.count_vertices()))
    }

    pub fn with_inserted_attribute<'py>(
        mut pyself: PyRefMut<'py, Self>,
        attribute: &PyMeshVertexAttribute,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attr_values = if let Ok(vref) = values.extract::<PyRef<PyVertexAttributeValues>>() {
            vref.0.clone()
        } else {
            PyVertexAttributeValues::new(values)?.0
        };

        // Validate COLOR attribute format before inserting
        if attribute.0 == Mesh::ATTRIBUTE_COLOR
            && !matches!(attr_values, VertexAttributeValues::Float32x4(_))
        {
            return Err(PyValueError::new_err(
                "Color attribute must have shape (N, 4)",
            ));
        }

        mesh_with_mut!(pyself, |mesh: &mut Mesh| {
            mesh.insert_attribute(attribute.0, attr_values);
        });

        Ok(pyself)
    }

    pub fn with_inserted_indices<'py>(
        mut pyself: PyRefMut<'py, Self>,
        indices: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        pyself.insert_indices(indices)?;
        Ok(pyself)
    }

    pub fn insert_attribute<'py>(
        &mut self,
        attribute: &PyMeshVertexAttribute,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let attr_values = if let Ok(vref) = values.extract::<PyRef<PyVertexAttributeValues>>() {
            vref.0.clone()
        } else {
            PyVertexAttributeValues::new(values)?.0
        };

        mesh_with_mut!(self, |mesh: &mut Mesh| {
            mesh.insert_attribute(attribute.0, attr_values);
        });

        Ok(())
    }

    pub fn insert_indices<'py>(&mut self, indices: &Bound<'py, PyAny>) -> PyResult<()> {
        let bevy_indices = if let Ok(vref) = indices.extract::<PyRef<PyIndices>>() {
            match &*vref {
                PyIndices::U16(vec) => Indices::U16(vec.clone()),
                PyIndices::U32(vec) => Indices::U32(vec.clone()),
            }
        } else if let Ok(arr_u32) = indices.extract::<PyReadonlyArray1<u32>>() {
            Indices::U32(arr_u32.as_slice()?.to_vec())
        } else if let Ok(arr_u16) = indices.extract::<PyReadonlyArray1<u16>>() {
            Indices::U16(arr_u16.as_slice()?.to_vec())
        } else if let Ok(vec32) = indices.extract::<Vec<u32>>() {
            Indices::U32(vec32)
        } else if let Ok(vec16) = indices.extract::<Vec<u16>>() {
            Indices::U16(vec16)
        } else {
            return Err(PyTypeError::new_err(
                "insert_indices expects Indices, a NumPy uint32/uint16 array, or a Python sequence of ints",
            ));
        };

        mesh_with_mut!(self, |mesh: &mut Mesh| {
            mesh.insert_indices(bevy_indices);
            Ok(())
        })
    }

    pub fn with_generated_tangents<'py>(
        mut pyself: PyRefMut<'py, Self>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        pyself.generate_tangents()?;
        Ok(pyself)
    }

    pub fn generate_tangents(&mut self) -> PyResult<()> {
        mesh_with_mut!(self, |mesh: &mut Mesh| { mesh.generate_tangents() })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to generate tangents: {}", e)))
    }

    pub fn positions(&self, py: Python<'_>) -> PyResult<Py<MeshAttributeContext>> {
        self.attribute(py, &PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_POSITION))
    }

    pub fn positions_mut(&mut self, py: Python<'_>) -> PyResult<Py<MeshAttributeContextMut>> {
        self.attribute_mut(py, &PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_POSITION))
    }

    pub fn normals(&self, py: Python<'_>) -> PyResult<Py<MeshAttributeContext>> {
        self.attribute(py, &PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_NORMAL))
    }

    pub fn normals_mut(&mut self, py: Python<'_>) -> PyResult<Py<MeshAttributeContextMut>> {
        self.attribute_mut(py, &PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_NORMAL))
    }

    pub fn uvs(&self, py: Python<'_>) -> PyResult<Py<MeshAttributeContext>> {
        self.attribute(py, &PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_UV_0))
    }

    pub fn uvs_mut(&mut self, py: Python<'_>) -> PyResult<Py<MeshAttributeContextMut>> {
        self.attribute_mut(py, &PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_UV_0))
    }

    pub fn attribute(
        &self,
        py: Python<'_>,
        id: &PyMeshVertexAttribute,
    ) -> PyResult<Py<MeshAttributeContext>> {
        mesh_with!(self, |mesh: &Mesh| {
            let Some(values) = mesh.attribute(id.0.id) else {
                return Err(PyRuntimeError::new_err(format!(
                    "Mesh does not have attribute {:?}",
                    id.0
                )));
            };

            let np_array = Self::create_numpy_view_readonly(py, values)?;
            let context = MeshAttributeContext { array: np_array };
            Py::new(py, context)
        })
    }

    pub fn attribute_mut(
        &mut self,
        py: Python<'_>,
        id: &PyMeshVertexAttribute,
    ) -> PyResult<Py<MeshAttributeContextMut>> {
        mesh_with_mut!(self, |mesh: &mut Mesh| {
            let Some(values) = mesh.attribute_mut(id.0.id) else {
                return Err(PyRuntimeError::new_err(format!(
                    "Mesh does not have attribute {:?}",
                    id.0
                )));
            };

            let np_array = PyMesh::create_numpy_view_mut(py, values)?;
            let context = MeshAttributeContextMut { array: np_array };
            Py::new(py, context)
        })
    }

    pub fn positions_copy(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f32>>> {
        mesh_with!(self, |mesh: &Mesh| {
            let Some(values) = mesh.attribute(Mesh::ATTRIBUTE_POSITION.id) else {
                return Err(PyRuntimeError::new_err("Mesh does not have positions"));
            };

            match values {
                VertexAttributeValues::Float32x3(v) => {
                    let n = v.len();
                    let flat: Vec<f32> = v.iter().flat_map(|p| p.iter().copied()).collect();
                    let array = PyArray2::from_vec2(py, &[flat])?.reshape([n, 3])?;
                    Ok(array.unbind())
                }
                _ => Err(PyRuntimeError::new_err("Positions must be Float32x3")),
            }
        })
    }

    pub fn set_positions(&mut self, positions: PyReadonlyArray2<f32>) -> PyResult<()> {
        mesh_with_mut!(self, |mesh: &mut Mesh| {
            let shape = positions.shape();
            if shape.len() != 2 || shape[1] != 3 {
                return Err(PyValueError::new_err("Positions must have shape (N, 3)"));
            }

            let data = positions.as_slice()?;
            let n = shape[0];
            let values: Vec<[f32; 3]> = (0..n)
                .map(|i| {
                    let idx = i * 3;
                    [data[idx], data[idx + 1], data[idx + 2]]
                })
                .collect();

            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, values);
            Ok(())
        })
    }

    pub fn normals_copy(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f32>>> {
        mesh_with!(self, |mesh: &Mesh| {
            let Some(values) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL.id) else {
                return Err(PyRuntimeError::new_err("Mesh does not have normals"));
            };

            match values {
                VertexAttributeValues::Float32x3(v) => {
                    let n = v.len();
                    let flat: Vec<f32> = v.iter().flat_map(|p| p.iter().copied()).collect();
                    let array = PyArray2::from_vec2(py, &[flat])?.reshape([n, 3])?;
                    Ok(array.unbind())
                }
                _ => Err(PyRuntimeError::new_err("Normals must be Float32x3")),
            }
        })
    }

    pub fn set_normals(&mut self, normals: PyReadonlyArray2<f32>) -> PyResult<()> {
        mesh_with_mut!(self, |mesh: &mut Mesh| {
            let shape = normals.shape();
            if shape.len() != 2 || shape[1] != 3 {
                return Err(PyValueError::new_err("Normals must have shape (N, 3)"));
            }

            let data = normals.as_slice()?;
            let n = shape[0];
            let values: Vec<[f32; 3]> = (0..n)
                .map(|i| {
                    let idx = i * 3;
                    [data[idx], data[idx + 1], data[idx + 2]]
                })
                .collect();

            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, values);
            Ok(())
        })
    }
}

// Helper methods for creating NumPy views (not exposed to Python)
impl PyMesh {
    fn create_numpy_view_readonly(
        py: Python<'_>,
        values: &VertexAttributeValues,
    ) -> PyResult<Py<PyAny>> {
        match values {
            VertexAttributeValues::Float32x2(v) => {
                let n = v.len();
                let flat: &[f32] =
                    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const f32, n * 2) };
                let view = ArrayView2::from_shape((n, 2), flat).unwrap();
                let np_array = unsafe {
                    PyArray2::borrow_from_array(&view, py.None().bind(py).clone().into_any())
                }
                .readwrite()
                .make_nonwriteable();
                Ok(Bound::clone(&np_array).unbind().into_any())
            }
            VertexAttributeValues::Float32x3(v) => {
                let n = v.len();
                let flat: &[f32] =
                    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const f32, n * 3) };
                let view = ArrayView2::from_shape((n, 3), flat).unwrap();
                let np_array = unsafe {
                    PyArray2::borrow_from_array(&view, py.None().bind(py).clone().into_any())
                }
                .readwrite()
                .make_nonwriteable();
                Ok(Bound::clone(&np_array).unbind().into_any())
            }
            VertexAttributeValues::Float32x4(v) => {
                let n = v.len();
                let flat: &[f32] =
                    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const f32, n * 4) };
                let view = ArrayView2::from_shape((n, 4), flat).unwrap();
                let np_array = unsafe {
                    PyArray2::borrow_from_array(&view, py.None().bind(py).clone().into_any())
                }
                .readwrite()
                .make_nonwriteable();
                Ok(Bound::clone(&np_array).unbind().into_any())
            }
            _ => Err(PyTypeError::new_err(
                "Unsupported attribute type for zero-copy access",
            )),
        }
    }

    fn create_numpy_view_mut(
        py: Python<'_>,
        values: &mut VertexAttributeValues,
    ) -> PyResult<Py<PyAny>> {
        match values {
            VertexAttributeValues::Float32x2(v) => {
                let n = v.len();
                let flat: &mut [f32] =
                    unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut f32, n * 2) };
                let view = ArrayViewMut2::from_shape((n, 2), flat).unwrap();
                let np = unsafe {
                    PyArray2::borrow_from_array(&view, py.None().bind(py).clone().into_any())
                };
                Ok(np.to_owned().unbind().into_any())
            }
            VertexAttributeValues::Float32x3(v) => {
                let n = v.len();
                let flat: &mut [f32] =
                    unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut f32, n * 3) };
                let view = ArrayViewMut2::from_shape((n, 3), flat).unwrap();
                let np = unsafe {
                    PyArray2::borrow_from_array(&view, py.None().bind(py).clone().into_any())
                };
                Ok(np.to_owned().unbind().into_any())
            }
            VertexAttributeValues::Float32x4(v) => {
                let n = v.len();
                let flat: &mut [f32] =
                    unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut f32, n * 4) };
                let view = ArrayViewMut2::from_shape((n, 4), flat).unwrap();
                let np = unsafe {
                    PyArray2::borrow_from_array(&view, py.None().bind(py).clone().into_any())
                };
                Ok(np.to_owned().unbind().into_any())
            }
            _ => Err(PyTypeError::new_err(
                "Unsupported attribute type for zero-copy mutable access",
            )),
        }
    }
}

#[pyclass(name = "MeshAttributeContext")]
pub struct MeshAttributeContext {
    array: Py<PyAny>,
}

#[pymethods]
impl MeshAttributeContext {
    fn __enter__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(slf.array.clone_ref(py))
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        Ok(false)
    }
}

#[pyclass(name = "MeshAttributeContextMut")]
pub struct MeshAttributeContextMut {
    array: Py<PyAny>,
}

#[pymethods]
impl MeshAttributeContextMut {
    fn __enter__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(slf.array.clone_ref(py))
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        Ok(false)
    }
}
