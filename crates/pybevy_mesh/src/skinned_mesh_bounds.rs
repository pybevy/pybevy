use bevy::mesh::skinning::{JointAabb, JointIndex, SkinnedMeshBounds};
use pybevy_core::{
    FieldStorage, FromBorrowedStorage, Omittable, ValueStorage, public_error, repr::ReprLiteral,
};
use pybevy_macros::{pyfield, pyvalue};
use pybevy_math::vec3::PyVec3;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyList};

use crate::mesh::PyMesh;

#[pyvalue]
#[pyclass(name = "JointIndex", module = "pybevy.mesh", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyJointIndex {
    pub(crate) storage: ValueStorage<JointIndex>,
}

impl PartialEq for PyJointIndex {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(a), Ok(b)) if *a == *b)
    }
}

impl From<JointIndex> for PyJointIndex {
    fn from(value: JointIndex) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyJointIndex> for JointIndex {
    type Error = PyErr;

    fn try_from(value: PyJointIndex) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyJointIndex> for JointIndex {
    type Error = PyErr;

    fn try_from(value: &PyJointIndex) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyJointIndex {
    #[new]
    pub fn new(value: u16) -> Self {
        Self::from_owned(JointIndex(value))
    }

    #[getter]
    pub fn value(&self) -> PyResult<u16> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: u16) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("JointIndex({})", self.as_ref()?.0))
    }
}

#[pyvalue]
#[pyclass(name = "JointAabb", module = "pybevy.mesh", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyJointAabb {
    pub(crate) storage: ValueStorage<JointAabb>,
}

impl PartialEq for PyJointAabb {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(a), Ok(b)) if *a == *b)
    }
}

impl From<JointAabb> for PyJointAabb {
    fn from(value: JointAabb) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyJointAabb> for JointAabb {
    type Error = PyErr;

    fn try_from(value: PyJointAabb) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyJointAabb> for JointAabb {
    type Error = PyErr;

    fn try_from(value: &PyJointAabb) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyJointAabb {
    #[new]
    #[pyo3(signature = (*, center, half_size))]
    pub fn new(center: PyVec3, half_size: PyVec3) -> PyResult<Self> {
        Ok(Self::from_owned(JointAabb {
            center: center.try_into()?,
            half_size: half_size.try_into()?,
        }))
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|aabb| &aabb.center, |aabb| &mut aabb.center)?)
    }

    #[setter]
    pub fn set_center(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.center = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn half_size(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|aabb| &aabb.half_size, |aabb| &mut aabb.half_size)?)
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.half_size = value.try_into()?;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let aabb = self.as_ref()?;
        Ok(format!(
            "JointAabb(center={}, half_size={})",
            aabb.center.repr_literal(),
            aabb.half_size.repr_literal()
        ))
    }
}

#[pyclass(name = "_JointAabbs", module = "pybevy.mesh", skip_from_py_object)]
#[derive(Clone)]
pub struct PyJointAabbs {
    storage: FieldStorage<Vec<JointAabb>>,
}

pybevy_core::impl_live_field_list!(
    PyJointAabbs,
    "_JointAabbs",
    Vec<JointAabb>,
    JointAabb,
    PyJointAabb,
    ValueStorage<JointAabb>
);

#[pyclass(name = "_JointIndices", module = "pybevy.mesh", skip_from_py_object)]
#[derive(Clone)]
pub struct PyJointIndices {
    storage: FieldStorage<Vec<JointIndex>>,
}

pybevy_core::impl_live_field_list!(
    PyJointIndices,
    "_JointIndices",
    Vec<JointIndex>,
    JointIndex,
    PyJointIndex,
    ValueStorage<JointIndex>
);

#[pyfield]
#[pyclass(name = "SkinnedMeshBounds", module = "pybevy.mesh", eq, from_py_object)]
pub struct PySkinnedMeshBounds {
    pub(crate) storage: FieldStorage<SkinnedMeshBounds>,
}

impl PartialEq for PySkinnedMeshBounds {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(a), Ok(b)) if *a == *b)
    }
}

fn joint_aabbs(values: Vec<PyJointAabb>) -> PyResult<Vec<JointAabb>> {
    values.into_iter().map(JointAabb::try_from).collect()
}

fn joint_indices(values: Vec<PyJointIndex>) -> PyResult<Vec<JointIndex>> {
    values.into_iter().map(JointIndex::try_from).collect()
}

#[pymethods]
impl PySkinnedMeshBounds {
    #[new]
    #[pyo3(signature = (*, aabbs = Omittable::default(), aabb_index_to_joint_index = Omittable::default()))]
    pub fn new(
        aabbs: Omittable<Vec<PyJointAabb>>,
        aabb_index_to_joint_index: Omittable<Vec<PyJointIndex>>,
    ) -> PyResult<Self> {
        Ok(Self::from_owned(SkinnedMeshBounds {
            aabbs: joint_aabbs(aabbs.into_option().unwrap_or_default())?,
            aabb_index_to_joint_index: joint_indices(
                aabb_index_to_joint_index.into_option().unwrap_or_default(),
            )?,
        }))
    }

    #[staticmethod]
    pub fn from_mesh(mesh: &PyMesh) -> PyResult<Self> {
        let bounds = SkinnedMeshBounds::from_mesh(&*mesh.as_ref()?).map_err(|error| {
            PyValueError::new_err(public_error::skinned_mesh_bounds_failed(
                "SkinnedMeshBounds.from_mesh",
                error,
            ))
        })?;
        Ok(Self::from_owned(bounds))
    }

    #[getter]
    pub fn aabbs(&self) -> PyResult<PyJointAabbs> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|bounds| &bounds.aabbs, |bounds| &mut bounds.aabbs)?)
    }

    #[setter]
    pub fn set_aabbs(&mut self, value: Vec<PyJointAabb>) -> PyResult<()> {
        let value = joint_aabbs(value)?;
        self.as_mut()?.aabbs = value;
        Ok(())
    }

    #[getter]
    pub fn aabb_index_to_joint_index(&self) -> PyResult<PyJointIndices> {
        Ok(self.storage.borrow_resolved_field_as(
            |bounds| &bounds.aabb_index_to_joint_index,
            |bounds| &mut bounds.aabb_index_to_joint_index,
        )?)
    }

    #[setter]
    pub fn set_aabb_index_to_joint_index(&mut self, value: Vec<PyJointIndex>) -> PyResult<()> {
        let value = joint_indices(value)?;
        self.as_mut()?.aabb_index_to_joint_index = value;
        Ok(())
    }

    /// Pairs of each joint index with its AABB, borrowed like the fields.
    pub fn iter(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let indices = self.aabb_index_to_joint_index()?;
        let aabbs = self.aabbs()?;
        let len = indices.__len__()?.min(aabbs.__len__()?);
        let pairs = (0..len as isize)
            .map(|index| Ok((indices.item(index)?, aabbs.item(index)?)))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(PyList::new(py, pairs)?.call_method0("__iter__")?.unbind())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "SkinnedMeshBounds(aabbs={})",
            self.as_ref()?.aabbs.len()
        ))
    }
}
