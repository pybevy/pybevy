//! Hierarchy component wrappers
//!
//! This module provides the Python bindings for Bevy's hierarchy components:
//! - ChildOf: Relationship component indicating parent entity
//! - Children: Auto-managed list of child entities (read-only)

use bevy::ecs::hierarchy::{ChildOf, Children};
use pyo3::{exceptions::PyIndexError, prelude::*};

use crate::{ComponentStorage, PyComponent, PyEntity, StorageMut, StorageRef};

#[pyclass(name = "ChildOf", module = "pybevy.ecs", extends = PyComponent, frozen)]
#[derive(Debug)]
pub struct PyChildOf {
    pub(crate) storage: ComponentStorage<ChildOf>,
}

impl From<ChildOf> for PyChildOf {
    fn from(component: ChildOf) -> Self {
        Self {
            storage: ComponentStorage::owned(component),
        }
    }
}

impl TryFrom<PyChildOf> for ChildOf {
    type Error = PyErr;

    fn try_from(py_component: PyChildOf) -> PyResult<Self> {
        Ok(py_component.storage.into_owned()?)
    }
}

impl TryFrom<&ChildOf> for PyChildOf {
    type Error = PyErr;

    fn try_from(component: &ChildOf) -> PyResult<Self> {
        Ok(Self {
            storage: ComponentStorage::owned(component.clone()),
        })
    }
}

impl PyChildOf {
    pub fn from_owned(component: ChildOf) -> (Self, PyComponent) {
        (
            Self {
                storage: ComponentStorage::owned(component),
            },
            PyComponent,
        )
    }

    pub fn from_borrowed(storage: ComponentStorage<ChildOf>) -> (Self, PyComponent) {
        (Self { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<StorageRef<'_, ChildOf>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<StorageMut<'_, ChildOf>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyChildOf {
    #[new]
    pub fn new(value: PyEntity) -> PyClassInitializer<Self> {
        Self::from_owned(ChildOf(value.0)).into()
    }

    /// Bevy's `ChildOf(pub Entity)` payload.
    ///
    /// Read-only, unlike the Rust field: Bevy's relationship hooks only run on
    /// insert/replace, so writing this in place would leave the old parent's
    /// `Children` stale. Replace the whole component to reparent.
    #[getter]
    pub fn value(&self) -> PyResult<PyEntity> {
        Ok(PyEntity(self.as_ref()?.0))
    }

    pub fn parent(&self) -> PyResult<PyEntity> {
        Ok(PyEntity(self.as_ref()?.parent()))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ChildOf({})", self.as_ref()?.parent()))
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()?.parent() == other.as_ref()?.parent())
    }
}

/// Auto-managed list of child entities.
///
/// Maintained by Bevy when ChildOf relationships change.
/// Not modifiable directly: add/remove ChildOf on children instead.
#[pyclass(name = "Children", module = "pybevy.ecs", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyChildren {
    entities: Vec<PyEntity>,
}

impl TryFrom<&Children> for PyChildren {
    type Error = PyErr;

    fn try_from(value: &Children) -> Result<Self, Self::Error> {
        // Children derefs to &[Entity], so we can iterate
        let entities = value.iter().map(|e| PyEntity(*e)).collect();
        Ok(PyChildren { entities })
    }
}

impl TryFrom<PyChildren> for Children {
    type Error = PyErr;

    fn try_from(_value: PyChildren) -> Result<Self, Self::Error> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Cannot create Children from Python - it is auto-managed by Bevy. Use ChildOf components to build hierarchies.",
        ))
    }
}

#[pymethods]
impl PyChildren {
    pub fn entities(&self) -> Vec<PyEntity> {
        self.entities.clone()
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyChildrenIterator {
        PyChildrenIterator {
            children: slf.entities.clone(),
            index: 0,
        }
    }

    fn __getitem__(&self, idx: isize) -> PyResult<PyEntity> {
        let len = self.entities.len() as isize;
        let actual_idx = if idx < 0 {
            (len + idx) as usize
        } else {
            idx as usize
        };

        self.entities
            .get(actual_idx)
            .copied()
            .ok_or_else(|| PyIndexError::new_err(format!("Index {} out of bounds", idx)))
    }

    fn __len__(&self) -> usize {
        self.entities.len()
    }

    fn __repr__(&self) -> String {
        format!("Children(count={})", self.entities.len())
    }
}

#[pyclass(name = "ChildrenIterator", module = "pybevy.ecs")]
pub struct PyChildrenIterator {
    children: Vec<PyEntity>,
    index: usize,
}

#[pymethods]
impl PyChildrenIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyEntity> {
        if slf.index < slf.children.len() {
            let entity = slf.children[slf.index];
            slf.index += 1;
            Some(entity)
        } else {
            None
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use bevy::ecs::{entity::Entity, world::World};

    use super::*;
    use crate::{storage_error::StorageError, validity_guard::ValidityFlag};

    /// Regression test: a ChildOf extracted as a read-only typed borrow must
    /// reject mutation even while the validity flag itself allows writes.
    #[test]
    fn borrowed_ref_child_of_rejects_mutation() {
        let mut world = World::new();
        let parent = world.spawn_empty().id();
        let component = ChildOf(parent);
        let flag = ValidityFlag::new_write();

        // SAFETY: component outlives the storage within this test scope
        let storage = unsafe { ComponentStorage::borrowed_ref(&component as *const ChildOf, flag) };
        let (mut py_child_of, _) = PyChildOf::from_borrowed(storage);

        assert_eq!(py_child_of.as_ref().unwrap().0, parent);
        assert!(matches!(
            py_child_of.storage.as_mut(),
            Err(StorageError::ReadOnly)
        ));
    }

    #[test]
    fn child_of_equality_and_repr_compare_the_parent_only() {
        let a = PyEntity(Entity::from_raw_u32(5).unwrap());
        let b = PyEntity(Entity::from_raw_u32(6).unwrap());
        let (first, _) = PyChildOf::from_owned(ChildOf(a.0));
        let (second, _) = PyChildOf::from_owned(ChildOf(a.0));
        let (other, _) = PyChildOf::from_owned(ChildOf(b.0));

        assert_eq!(first.value().unwrap(), a);
        assert_eq!(first.parent().unwrap(), a);
        assert!(first.__eq__(&second).unwrap());
        assert!(!first.__eq__(&other).unwrap());
        // Bevy 0.19 prints entities as "<index>v<generation>".
        assert_eq!(first.__repr__().unwrap(), "ChildOf(5v0)");
    }

    #[test]
    fn children_observations_pin_exact_indexes_repr_and_errors() {
        // The real producer of Children is Bevy's relationship hook on spawn.
        let mut world = World::new();
        let parent = world.spawn_empty().id();
        let e1 = world.spawn(ChildOf(parent)).id();
        let e2 = world.spawn(ChildOf(parent)).id();
        let e3 = world.spawn(ChildOf(parent)).id();

        let children = world.entity(parent).get::<Children>().unwrap();
        let py_children: PyChildren = children.try_into().unwrap();

        assert_eq!(py_children.__repr__(), "Children(count=3)");
        assert_eq!(py_children.len(), 3);
        assert!(!py_children.is_empty());
        assert_eq!(
            py_children.entities(),
            vec![PyEntity(e1), PyEntity(e2), PyEntity(e3)]
        );
        // Positive, last, and negative indexes map onto the stored order.
        assert_eq!(py_children.__getitem__(0).unwrap(), PyEntity(e1));
        assert_eq!(py_children.__getitem__(2).unwrap(), PyEntity(e3));
        assert_eq!(py_children.__getitem__(-1).unwrap(), PyEntity(e3));

        Python::attach(|py| {
            for (idx, message) in [(3, "Index 3 out of bounds"), (-4, "Index -4 out of bounds")] {
                let error = py_children.__getitem__(idx).unwrap_err();
                let value = error.value(py);
                assert!(value.is_instance_of::<PyIndexError>());
                assert_eq!(value.str().unwrap().to_string(), message);
            }
        });
    }
}
