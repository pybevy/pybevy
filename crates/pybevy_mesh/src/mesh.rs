use std::{fmt::Display, sync::Arc};

use bevy::{
    asset::RenderAssetUsages,
    math::{Quat, Vec3},
    mesh::{Indices, Mesh, MeshVertexAttributeId, PrimitiveTopology, VertexAttributeValues},
    transform::components::Transform,
};
use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pybevy_array::{BorrowProbe, PyArray, borrowed_read_only_f32};
use pybevy_core::{
    AssetInputConverter, AssetStorage, PyAsset,
    borrowed_array_anchor::{AssetBorrowAnchor, AssetBorrowAnchorMut},
    content_hash::CanonicalContentHasher,
    numpy_view_guard::{PendingNumpyViewGuard, PyNumpyViewGuard},
};
use pybevy_image::image::PyRenderAssetUsages;
use pybevy_macros::pyasset;
use pybevy_math::{quat::PyQuat, vec3::PyVec3};
use pybevy_transform::transform::PyTransform;
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::PyBytes,
};

use crate::{
    indices::PyIndices,
    mesh_builder::PyMeshBuilder,
    meshable::{PyMeshable, meshable_to_mesh},
    primitive_topology::PyPrimitiveTopology,
    vertex_attribute::{PyMeshVertexAttribute, PyVertexAttributeValues, attribute_id},
};
#[pyasset(Mesh, bridge, input_converter)]
#[pyclass(name = "Mesh", module = "pybevy.mesh", extends = PyAsset, skip_from_py_object)]
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

fn extract_attribute_values(
    attribute: &PyMeshVertexAttribute,
    values: &Bound<'_, PyAny>,
) -> PyResult<VertexAttributeValues> {
    let attr_values = if let Ok(vref) = values.extract::<PyRef<PyVertexAttributeValues>>() {
        vref.0.clone()
    } else {
        PyVertexAttributeValues::new(values)?.0
    };

    if attribute.0 == Mesh::ATTRIBUTE_COLOR
        && !matches!(attr_values, VertexAttributeValues::Float32x4(_))
    {
        return Err(PyValueError::new_err(
            "Color attribute must have shape (N, 4)",
        ));
    }

    Ok(attr_values)
}

// bevy validates these preconditions with assert!, not Err: converting them to
// ValueError so a bad mesh is a catchable error rather than a frame panic.
fn check_flat_normal_preconditions(mesh: &Mesh) -> PyResult<()> {
    let indexed = mesh
        .try_indices_option()
        .map_err(|error| mesh_operation_error("compute_flat_normals", error))?
        .is_some();
    if indexed {
        return Err(PyValueError::new_err(
            "compute_flat_normals requires non-indexed geometry; call duplicate_vertices() \
             first, or use compute_smooth_normals()",
        ));
    }
    let topology = mesh.primitive_topology();
    if topology != PrimitiveTopology::TriangleList {
        return Err(PyValueError::new_err(format!(
            "compute_flat_normals requires PrimitiveTopology.TriangleList (got {topology:?})"
        )));
    }
    Ok(())
}

// bevy asserts indexed + TriangleList here; surface as ValueError, not a panic.
fn check_area_weighted_normal_preconditions(mesh: &Mesh) -> PyResult<()> {
    let indexed = mesh
        .try_indices_option()
        .map_err(|error| mesh_operation_error("compute_area_weighted_normals", error))?
        .is_some();
    if !indexed {
        return Err(PyValueError::new_err(
            "compute_area_weighted_normals requires indexed geometry; use compute_flat_normals() \
             on non-indexed geometry instead",
        ));
    }
    let topology = mesh.primitive_topology();
    if topology != PrimitiveTopology::TriangleList {
        return Err(PyValueError::new_err(format!(
            "compute_area_weighted_normals requires PrimitiveTopology.TriangleList (got {topology:?})"
        )));
    }
    Ok(())
}

// bevy asserts indexed + TriangleList here; surface as ValueError, not a panic.
fn check_smooth_normal_preconditions(mesh: &Mesh) -> PyResult<()> {
    let indexed = mesh
        .try_indices_option()
        .map_err(|error| mesh_operation_error("compute_custom_smooth_normals", error))?
        .is_some();
    if !indexed {
        return Err(PyValueError::new_err(
            "compute_custom_smooth_normals requires indexed geometry; use compute_flat_normals() \
             on non-indexed geometry instead",
        ));
    }
    let topology = mesh.primitive_topology();
    if topology != PrimitiveTopology::TriangleList {
        return Err(PyValueError::new_err(format!(
            "compute_custom_smooth_normals requires PrimitiveTopology.TriangleList (got {topology:?})"
        )));
    }
    Ok(())
}

macro_rules! mesh_with {
    ($s:expr, $f:expr) => {{
        let mesh = (*$s).as_ref()?;
        $f(&mesh)
    }};
}

macro_rules! mesh_with_mut {
    ($s:expr, $f:expr) => {{
        let mut mesh = (*$s).as_mut()?;
        $f(&mut mesh)
    }};
}

/// Normalize a positions/normals argument to a contiguous `(N, 3)` f32 array:
/// accepts real NumPy, the bounded `pybevy.array` array (via `__array__`), and
/// (nested) lists, matching `insert_indices`' portable input surface.
fn asarray_2d_f32<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<PyReadonlyArray2<'py, f32>> {
    let np = py.import("numpy")?;
    let arr = np.call_method1("asarray", (obj,))?;
    let arr = arr.call_method1("astype", (np.getattr("float32")?,))?;
    Ok(arr.extract::<PyReadonlyArray2<f32>>()?)
}

fn mesh_operation_error(operation: &str, error: impl Display) -> PyErr {
    PyRuntimeError::new_err(format!("Mesh.{operation}() failed: {error}"))
}

fn mesh_payload_hash(mesh: &Mesh) -> PyResult<String> {
    #[cfg(target_endian = "big")]
    return Err(PyRuntimeError::new_err(
        "Mesh._content_hash() does not yet support big-endian targets",
    ));

    #[cfg(target_endian = "little")]
    {
        let attributes = mesh
            .try_attributes()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let mut attributes: Vec<_> = attributes
            .map(|(attribute, values)| (attribute_id(attribute), attribute, values))
            .collect();
        attributes.sort_by_key(|(id, _, _)| *id);

        let mut hasher = CanonicalContentHasher::new("pybevy.mesh.payload", 1);
        for (id, attribute, values) in attributes {
            hasher.write("attribute.id", &id.to_le_bytes());
            hasher.write(
                "attribute.format",
                format!("{:?}", attribute.format).as_bytes(),
            );
            hasher.write(
                "attribute.vertex_count",
                &(values.len() as u64).to_le_bytes(),
            );
            hasher.write("attribute.bytes", values.get_bytes());
        }
        hasher.write(
            "primitive_topology",
            format!("{:?}", mesh.primitive_topology()).as_bytes(),
        );

        match mesh
            .try_indices_option()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?
        {
            None => hasher.write("indices.none", &[]),
            Some(Indices::U16(indices)) => {
                let mut bytes = Vec::with_capacity(indices.len() * size_of::<u16>());
                for index in indices {
                    bytes.extend_from_slice(&index.to_le_bytes());
                }
                hasher.write("indices.u16", &bytes);
            }
            Some(Indices::U32(indices)) => {
                let mut bytes = Vec::with_capacity(indices.len() * size_of::<u32>());
                for index in indices {
                    bytes.extend_from_slice(&index.to_le_bytes());
                }
                hasher.write("indices.u32", &bytes);
            }
        }

        Ok(hasher.finish())
    }
}

impl PyMesh {
    /// Clone the mesh, apply an in-place operation to the copy, and return the copy.
    fn cloned_with(
        &self,
        py: Python<'_>,
        apply: impl FnOnce(&mut Self) -> PyResult<()>,
    ) -> PyResult<Py<Self>> {
        let copy = Py::new(py, Self::from_owned(self.as_ref()?.clone()))?;
        {
            let mut guard = copy.borrow_mut(py);
            apply(&mut guard)?;
        }
        Ok(copy)
    }
}

#[pymethods]
impl PyMesh {
    #[classattr]
    #[pyo3(name = "ATTRIBUTE_POSITION")]
    pub fn attribute_position(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_POSITION))
    }

    #[classattr]
    #[pyo3(name = "ATTRIBUTE_NORMAL")]
    pub fn attribute_normal(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_NORMAL))
    }

    #[classattr]
    #[pyo3(name = "ATTRIBUTE_UV_0")]
    pub fn attribute_uv_0(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_UV_0))
    }

    #[classattr]
    #[pyo3(name = "ATTRIBUTE_UV_1")]
    pub fn attribute_uv_1(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_UV_1))
    }

    #[classattr]
    #[pyo3(name = "ATTRIBUTE_TANGENT")]
    pub fn attribute_tangent(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_TANGENT))
    }

    #[classattr]
    #[pyo3(name = "ATTRIBUTE_COLOR")]
    pub fn attribute_color(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_COLOR))
    }

    #[classattr]
    #[pyo3(name = "ATTRIBUTE_JOINT_WEIGHT")]
    pub fn attribute_joint_weight(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(
            py,
            PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_JOINT_WEIGHT),
        )
    }

    #[classattr]
    #[pyo3(name = "ATTRIBUTE_JOINT_INDEX")]
    pub fn attribute_joint_index(py: Python<'_>) -> PyResult<Py<PyMeshVertexAttribute>> {
        Py::new(py, PyMeshVertexAttribute::from(Mesh::ATTRIBUTE_JOINT_INDEX))
    }

    #[new]
    pub fn new(primitive_topology: PyPrimitiveTopology) -> PyClassInitializer<Self> {
        Self::from_owned(Mesh::new(
            primitive_topology.into(),
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        ))
        .into()
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

    #[setter]
    pub fn set_asset_usage(&mut self, usage: PyRenderAssetUsages) -> PyResult<()> {
        let usage = usage.into();
        mesh_with_mut!(self, |mesh: &mut Mesh| mesh.asset_usage = usage);
        Ok(())
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

    pub fn create_packed_vertex_buffer_data<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        mesh_with!(self, |mesh: &Mesh| {
            Ok(PyBytes::new(py, &mesh.create_packed_vertex_buffer_data()))
        })
    }

    pub fn contains_attribute(&self, attribute: &PyMeshVertexAttribute) -> PyResult<bool> {
        mesh_with!(self, |mesh: &Mesh| mesh
            .try_contains_attribute(attribute.0.id))
        .map_err(|error| mesh_operation_error("contains_attribute", error))
    }

    pub fn remove_attribute(
        &mut self,
        attribute: &PyMeshVertexAttribute,
    ) -> PyResult<Option<PyVertexAttributeValues>> {
        let present = mesh_with!(self, |mesh: &Mesh| mesh
            .try_contains_attribute(attribute.0.id))
        .map_err(|error| mesh_operation_error("remove_attribute", error))?;
        if !present {
            return Ok(None);
        }
        let values = mesh_with_mut!(self, |mesh: &mut Mesh| mesh
            .try_remove_attribute(attribute.0.id)
            .expect("read preflight confirmed mutable attribute storage and presence"));
        Ok(Some(values.into()))
    }

    pub fn with_removed_attribute(
        &self,
        py: Python<'_>,
        attribute: &PyMeshVertexAttribute,
    ) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.remove_attribute(attribute).map(|_| ()))
    }

    /// Return the versioned SHA-256 digest of vertex attributes, topology, and indices.
    pub fn _content_hash(&self) -> PyResult<String> {
        mesh_with!(self, mesh_payload_hash)
    }

    pub fn with_inserted_attribute<'py>(
        &self,
        py: Python<'py>,
        attribute: &PyMeshVertexAttribute,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<Py<Self>> {
        let attr_values = extract_attribute_values(attribute, values)?;
        let mut mesh = self.as_ref()?.clone();
        mesh.insert_attribute(attribute.0, attr_values);
        Py::new(py, Self::from_owned(mesh))
    }

    pub fn with_inserted_indices<'py>(
        &self,
        py: Python<'py>,
        indices: &Bound<'py, PyAny>,
    ) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.insert_indices(indices))
    }

    pub fn insert_attribute<'py>(
        &mut self,
        attribute: &PyMeshVertexAttribute,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let attr_values = extract_attribute_values(attribute, values)?;

        mesh_with_mut!(self, |mesh: &mut Mesh| {
            mesh.insert_attribute(attribute.0, attr_values);
        });

        Ok(())
    }

    pub fn insert_indices<'py>(&mut self, indices: &Bound<'py, PyAny>) -> PyResult<()> {
        let bevy_indices = if let Ok(vref) = indices.extract::<PyRef<PyIndices>>() {
            vref.inner.clone()
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

    pub fn with_generated_tangents(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.generate_tangents())
    }

    pub fn generate_tangents(&mut self) -> PyResult<()> {
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        candidate
            .generate_tangents()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to generate tangents: {e}")))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    pub fn compute_normals(&mut self) -> PyResult<()> {
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        candidate
            .try_compute_normals()
            .map_err(|error| mesh_operation_error("compute_normals", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    pub fn compute_area_weighted_normals(&mut self) -> PyResult<()> {
        let mesh = self.as_ref()?;
        check_area_weighted_normal_preconditions(&mesh)?;
        drop(mesh);
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        candidate
            .try_compute_area_weighted_normals()
            .map_err(|error| mesh_operation_error("compute_area_weighted_normals", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    pub fn compute_flat_normals(&mut self) -> PyResult<()> {
        let mesh = self.as_ref()?;
        check_flat_normal_preconditions(&mesh)?;
        drop(mesh);
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        candidate
            .try_compute_flat_normals()
            .map_err(|error| mesh_operation_error("compute_flat_normals", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    pub fn compute_smooth_normals(&mut self) -> PyResult<()> {
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        candidate
            .try_compute_smooth_normals()
            .map_err(|error| mesh_operation_error("compute_smooth_normals", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    /// Per-triangle Python callback mutating the accumulating normals list in place.
    #[pyo3(signature = (per_triangle, *))]
    pub fn compute_custom_smooth_normals<'py>(
        &mut self,
        py: Python<'py>,
        per_triangle: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let mesh = self.as_ref()?;
        check_smooth_normal_preconditions(&mesh)?;
        drop(mesh);
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        let mut caught: Option<PyErr> = None;
        candidate
            .try_compute_custom_smooth_normals(|[a, b, c], positions, normals| {
                if caught.is_some() {
                    return;
                }
                let indices = (a, b, c);
                let positions_list: Vec<(f32, f32, f32)> =
                    positions.iter().map(|p| (p[0], p[1], p[2])).collect();
                let normals_py =
                    pyo3::types::PyList::new(py, normals.iter().map(|n| (n.x, n.y, n.z)))
                        .expect("tuple list conversion cannot fail");
                match per_triangle.call1((indices, positions_list, &normals_py)) {
                    Ok(_) => {
                        for (index, slot) in normals.iter_mut().enumerate() {
                            let value = normals_py
                                .get_item(index)
                                .and_then(|item| item.extract::<(f32, f32, f32)>());
                            match value {
                                Ok((x, y, z)) => *slot = bevy::math::Vec3::new(x, y, z),
                                Err(error) => {
                                    caught = Some(error);
                                    return;
                                }
                            }
                        }
                    }
                    Err(error) => caught = Some(error),
                }
            })
            .map_err(|error| mesh_operation_error("compute_custom_smooth_normals", error))?;
        if let Some(error) = caught {
            return Err(error);
        }
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    pub fn with_computed_normals(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.compute_normals())
    }

    pub fn with_computed_flat_normals(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.compute_flat_normals())
    }

    pub fn with_computed_smooth_normals(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.compute_smooth_normals())
    }

    pub fn duplicate_vertices(&mut self) -> PyResult<()> {
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        candidate
            .try_duplicate_vertices()
            .map_err(|error| mesh_operation_error("duplicate_vertices", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    pub fn with_duplicated_vertices(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.duplicate_vertices())
    }

    pub fn invert_winding(&mut self) -> PyResult<()> {
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        candidate
            .invert_winding()
            .map_err(|error| mesh_operation_error("invert_winding", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    pub fn with_inverted_winding(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.invert_winding())
    }

    pub fn merge(&mut self, other: &PyMesh) -> PyResult<()> {
        let other = other.storage.as_ref()?.clone();
        let mut candidate = mesh_with!(self, |mesh: &Mesh| mesh.clone());
        candidate
            .merge(&other)
            .map_err(|error| mesh_operation_error("merge", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| *mesh = candidate);
        Ok(())
    }

    pub fn transform_by(&mut self, transform: &PyTransform) -> PyResult<()> {
        let transform: Transform = *transform.as_ref()?;
        mesh_with!(self, |mesh: &Mesh| mesh
            .try_contains_attribute(Mesh::ATTRIBUTE_POSITION.id))
        .map_err(|error| mesh_operation_error("transform_by", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| mesh
            .try_transform_by(transform)
            .expect("read preflight confirmed mutable attribute storage"));
        Ok(())
    }

    pub fn transformed_by(&self, py: Python<'_>, transform: &PyTransform) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.transform_by(transform))
    }

    pub fn translate_by(&mut self, translation: PyVec3) -> PyResult<()> {
        let translation: Vec3 = translation.try_into()?;
        mesh_with!(self, |mesh: &Mesh| mesh
            .try_contains_attribute(Mesh::ATTRIBUTE_POSITION.id))
        .map_err(|error| mesh_operation_error("translate_by", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| mesh
            .try_translate_by(translation)
            .expect("read preflight confirmed mutable attribute storage"));
        Ok(())
    }

    pub fn translated_by(&self, py: Python<'_>, translation: PyVec3) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.translate_by(translation))
    }

    pub fn rotate_by(&mut self, rotation: PyQuat) -> PyResult<()> {
        let rotation: Quat = rotation.try_into()?;
        mesh_with!(self, |mesh: &Mesh| mesh
            .try_contains_attribute(Mesh::ATTRIBUTE_POSITION.id))
        .map_err(|error| mesh_operation_error("rotate_by", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| mesh
            .try_rotate_by(rotation)
            .expect("read preflight confirmed mutable attribute storage"));
        Ok(())
    }

    pub fn rotated_by(&self, py: Python<'_>, rotation: PyQuat) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.rotate_by(rotation))
    }

    pub fn scale_by(&mut self, scale: PyVec3) -> PyResult<()> {
        let scale: Vec3 = scale.try_into()?;
        mesh_with!(self, |mesh: &Mesh| mesh
            .try_contains_attribute(Mesh::ATTRIBUTE_POSITION.id))
        .map_err(|error| mesh_operation_error("scale_by", error))?;
        mesh_with_mut!(self, |mesh: &mut Mesh| mesh
            .try_scale_by(scale)
            .expect("read preflight confirmed mutable attribute storage"));
        Ok(())
    }

    pub fn scaled_by(&self, py: Python<'_>, scale: PyVec3) -> PyResult<Py<Self>> {
        self.cloned_with(py, |mesh| mesh.scale_by(scale))
    }

    pub fn has_morph_targets(&self) -> PyResult<bool> {
        mesh_with!(self, |mesh: &Mesh| mesh.try_has_morph_targets())
            .map_err(|error| mesh_operation_error("has_morph_targets", error))
    }

    /// Read-only bounded array of the position attribute, shape `(N, 3)`.
    ///
    /// Borrows the mesh vertex data zero-copy: mutation of the mesh is blocked
    /// while the array is alive, and after the owning system ends every
    /// operation raises a clean `RuntimeError`. Call `.to_numpy()` (or `.copy()`)
    /// for an independent snapshot. The surface is portable across backends.
    pub fn positions(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyArray>> {
        Self::attribute_borrowed(slf, py, Mesh::ATTRIBUTE_POSITION.id)
    }

    /// In-place mutable bounded-array context over vertex positions.
    ///
    /// ```python
    /// with mesh.positions_mut() as pos:
    ///     pos[:, 1] += 1.0   # writes land directly in the mesh
    /// ```
    /// Writes are immediate (zero-copy); the mesh is change-marked on entry, and
    /// after the block the array is closed. In-place on both backends.
    pub fn positions_mut(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
    ) -> PyResult<Py<MeshBoundedContextMut>> {
        Self::attribute_borrowed_mut(slf, py, Mesh::ATTRIBUTE_POSITION.id)
    }

    /// Read-only zero-copy bounded array of the normal attribute.
    pub fn normals(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyArray>> {
        Self::attribute_borrowed(slf, py, Mesh::ATTRIBUTE_NORMAL.id)
    }

    /// In-place mutable bounded-array context over vertex normals.
    pub fn normals_mut(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
    ) -> PyResult<Py<MeshBoundedContextMut>> {
        Self::attribute_borrowed_mut(slf, py, Mesh::ATTRIBUTE_NORMAL.id)
    }

    /// Read-only zero-copy bounded array of the UV0 attribute.
    pub fn uvs(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyArray>> {
        Self::attribute_borrowed(slf, py, Mesh::ATTRIBUTE_UV_0.id)
    }

    /// In-place mutable bounded-array context over UV0 coordinates.
    pub fn uvs_mut(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<MeshBoundedContextMut>> {
        Self::attribute_borrowed_mut(slf, py, Mesh::ATTRIBUTE_UV_0.id)
    }

    /// Read-only zero-copy bounded array of an arbitrary float32 vertex attribute.
    pub fn attribute(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        id: &PyMeshVertexAttribute,
    ) -> PyResult<Py<PyArray>> {
        Self::attribute_borrowed(slf, py, id.0.id)
    }

    /// In-place mutable bounded-array context over an arbitrary float32 attribute.
    pub fn attribute_mut(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        id: &PyMeshVertexAttribute,
    ) -> PyResult<Py<MeshBoundedContextMut>> {
        Self::attribute_borrowed_mut(slf, py, id.0.id)
    }

    pub fn set_positions(&mut self, py: Python<'_>, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = asarray_2d_f32(py, positions)?;
        let shape = positions.shape();
        if shape.len() != 2 || shape[1] != 3 {
            return Err(PyValueError::new_err("Positions must have shape (N, 3)"));
        }
        let data = positions.as_slice()?;
        let values: Vec<[f32; 3]> = data
            .chunks_exact(3)
            .map(|row| [row[0], row[1], row[2]])
            .collect();
        mesh_with_mut!(self, |mesh: &mut Mesh| {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, values);
        });
        Ok(())
    }

    pub fn set_normals(&mut self, py: Python<'_>, normals: &Bound<'_, PyAny>) -> PyResult<()> {
        let normals = asarray_2d_f32(py, normals)?;
        let shape = normals.shape();
        if shape.len() != 2 || shape[1] != 3 {
            return Err(PyValueError::new_err("Normals must have shape (N, 3)"));
        }
        let data = normals.as_slice()?;
        let values: Vec<[f32; 3]> = data
            .chunks_exact(3)
            .map(|row| [row[0], row[1], row[2]])
            .collect();
        mesh_with_mut!(self, |mesh: &mut Mesh| {
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, values);
        });
        Ok(())
    }
}

impl PyMesh {
    /// Build a read-only, zero-copy bounded array over a float32 vertex
    /// attribute, guarded so mutation is blocked while it is alive and access
    /// after the owning system ends fails cleanly.
    fn attribute_borrowed(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        id: MeshVertexAttributeId,
    ) -> PyResult<Py<PyArray>> {
        let this = slf.borrow();
        // Atomically claim a read view (increment + verify no writer), closing
        // the check-then-acquire race under free-threading. On any later error
        // the guard's Drop releases it.
        let claim = this.storage.prepare_read_view()?;
        let guard = PyNumpyViewGuard::from_acquired(claim, slf.clone().unbind().into_any());
        let validity = this.storage.validity_flag();
        let mesh = this.storage.as_ref()?;
        let Some(values) = mesh.attribute(id) else {
            return Err(PyRuntimeError::new_err("Mesh does not have attribute"));
        };
        let (ptr, len, shape) = attribute_f32_view(values)?;
        let probe: Arc<dyn BorrowProbe> = Arc::new(AssetBorrowAnchor::new(validity, guard));
        // SAFETY: `ptr` comes from a live `[f32; N]`-backed attribute obtained
        // through `as_ref()` (validity just checked). `[f32; N]` is contiguous
        // with no padding, so `n` elements view as `n * N` `f32`s (same proof as
        // the NumPy zero-copy views). The probe carries the same ValidityFlag
        // gating the borrow and holds the read-view count acquired above, so the
        // mesh cannot be reallocated through this wrapper or read after the
        // system ends while the array is alive.
        let nd = unsafe { borrowed_read_only_f32(ptr, len, &shape, probe)? };
        Py::new(py, nd)
    }

    /// Build an in-place mutable bounded-array context over a float32 attribute.
    fn attribute_borrowed_mut(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        id: MeshVertexAttributeId,
    ) -> PyResult<Py<MeshBoundedContextMut>> {
        let mut this = slf.borrow_mut();
        let layout = {
            let mesh = this.storage.as_ref()?;
            let values = mesh
                .attribute(id)
                .ok_or_else(|| PyRuntimeError::new_err("Mesh does not have attribute"))?;
            attribute_f32_layout(values)?
        };
        let validity = this.storage.validity_flag();
        let claim = this.storage.prepare_write_view()?;
        let guard = PendingNumpyViewGuard::from_acquired(claim, slf.clone().unbind().into_any());
        let anchor = Arc::new(AssetBorrowAnchorMut::new(validity, guard));
        let probe: Arc<dyn BorrowProbe> = anchor.clone();
        let array = Py::new(
            py,
            PyArray::pending_borrowed_mut_f32(layout.len, layout.shape(), probe)?,
        )?;
        let context = Py::new(
            py,
            MeshBoundedContextMut {
                array: array.clone_ref(py),
                anchor: anchor.clone(),
            },
        )?;

        let mut transaction = this.storage.begin_write_view(anchor.pending_claim())?;
        let current = transaction
            .preflight()
            .attribute(id)
            .ok_or_else(|| PyRuntimeError::new_err("Mesh does not have attribute"))?;
        if attribute_f32_layout(current)? != layout {
            return Err(PyRuntimeError::new_err(
                "Mesh attribute layout changed during view acquisition",
            ));
        }

        let mesh = transaction.commit();
        let values = mesh
            .attribute_mut(id)
            .expect("preflight confirmed the mesh attribute exists");
        let ptr = attribute_f32_ptr_mut(values, layout);
        {
            let mut pending = array.borrow_mut(py);
            // SAFETY: the committed transaction returned the same validated
            // attribute under the exclusive view claim retained by `anchor`.
            unsafe { pending.bind_borrowed_mut_f32(ptr) };
        };
        anchor.commit();
        drop(transaction);
        Ok(context)
    }
}

/// A raw `f32` view of a float32 vertex attribute: `(ptr, element_count, shape)`.
/// Scalar attributes are 1-D; vector attributes are `(vertices, components)`.
fn attribute_f32_view(values: &VertexAttributeValues) -> PyResult<(*const f32, usize, Vec<usize>)> {
    match values {
        VertexAttributeValues::Float32(v) => Ok((v.as_ptr(), v.len(), vec![v.len()])),
        VertexAttributeValues::Float32x2(v) => {
            Ok((v.as_ptr() as *const f32, v.len() * 2, vec![v.len(), 2]))
        }
        VertexAttributeValues::Float32x3(v) => {
            Ok((v.as_ptr() as *const f32, v.len() * 3, vec![v.len(), 3]))
        }
        VertexAttributeValues::Float32x4(v) => {
            Ok((v.as_ptr() as *const f32, v.len() * 4, vec![v.len(), 4]))
        }
        _ => Err(PyRuntimeError::new_err(
            "only float32 attributes are exposed as bounded arrays",
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FloatAttributeLayout {
    len: usize,
    shape: [usize; 2],
    ndim: usize,
}

impl FloatAttributeLayout {
    fn shape(&self) -> &[usize] {
        &self.shape[..self.ndim]
    }
}

fn attribute_f32_layout(values: &VertexAttributeValues) -> PyResult<FloatAttributeLayout> {
    match values {
        VertexAttributeValues::Float32(v) => Ok(FloatAttributeLayout {
            len: v.len(),
            shape: [v.len(), 0],
            ndim: 1,
        }),
        VertexAttributeValues::Float32x2(v) => Ok(FloatAttributeLayout {
            len: v.len() * 2,
            shape: [v.len(), 2],
            ndim: 2,
        }),
        VertexAttributeValues::Float32x3(v) => Ok(FloatAttributeLayout {
            len: v.len() * 3,
            shape: [v.len(), 3],
            ndim: 2,
        }),
        VertexAttributeValues::Float32x4(v) => Ok(FloatAttributeLayout {
            len: v.len() * 4,
            shape: [v.len(), 4],
            ndim: 2,
        }),
        _ => Err(PyRuntimeError::new_err(
            "only float32 attributes are exposed as bounded arrays",
        )),
    }
}

fn attribute_f32_ptr_mut(
    values: &mut VertexAttributeValues,
    layout: FloatAttributeLayout,
) -> *mut f32 {
    match (values, layout.shape()) {
        (VertexAttributeValues::Float32(values), [_]) => values.as_mut_ptr(),
        (VertexAttributeValues::Float32x2(values), [_, 2]) => values.as_mut_ptr().cast(),
        (VertexAttributeValues::Float32x3(values), [_, 3]) => values.as_mut_ptr().cast(),
        (VertexAttributeValues::Float32x4(values), [_, 4]) => values.as_mut_ptr().cast(),
        _ => unreachable!("preflight fixed the mesh attribute layout"),
    }
}

/// Context manager yielding an in-place mutable bounded array over a mesh
/// attribute. Writes land directly in the mesh; on exit the array is closed
/// (further reads/writes raise) and the exclusive write count released.
#[pyclass(name = "MeshBoundedContextMut", module = "pybevy.mesh")]
pub struct MeshBoundedContextMut {
    array: Py<PyArray>,
    anchor: Arc<AssetBorrowAnchorMut>,
}

#[pymethods]
impl MeshBoundedContextMut {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.array)
    }

    fn __enter__(&self, py: Python<'_>) -> Py<PyArray> {
        self.array.clone_ref(py)
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        // Close the borrow: writes made before an exception are already applied
        // in place; the array becomes unusable and the write count releases.
        self.anchor.close();
        false
    }
}
